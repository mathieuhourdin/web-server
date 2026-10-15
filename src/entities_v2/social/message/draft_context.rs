//! Saved draft + continuation context for owner-to-mentor questions.
//! Keep raw snapshots private; only the small provenance object is exposed on messages.
use crate::{
    db::DbPool,
    entities_v2::error::{ErrorType, PpdcError},
};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Integer, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

const MAX_CONTEXT_CHARS: usize = 100_000;
const MAX_THREAD_TRACES: i64 = 200;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DraftMentorSource {
    pub trace_id: Uuid,
    pub trace_version_integer: i32,
    pub writing_thread_id: Option<Uuid>,
    pub thread_input_revision: Option<i64>,
    pub omitted_thread_trace_count: i64,
}

pub struct PreparedDraftContext {
    pub source: DraftMentorSource,
    pub input: Value,
}

impl PreparedDraftContext {
    pub fn metadata(&self) -> super::model::MessageMetadata {
        super::model::MessageMetadata {
            mentor_feedback: None,
            draft_trace: Some(self.source.clone()),
        }
    }
}

#[derive(QueryableByName, Serialize)]
struct Draft {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    subtitle: String,
    #[diesel(sql_type = Text)]
    content: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Text)]
    trace_type: String,
    #[diesel(sql_type = Bool)]
    is_encrypted: bool,
    #[diesel(sql_type = Integer)]
    version_integer: i32,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    continues_trace_id: Option<Uuid>,
    #[diesel(sql_type = Bool)]
    journal_available: bool,
}

#[derive(QueryableByName, Serialize)]
struct Thread {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = BigInt)]
    input_revision: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    compiled_revision: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    title: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    ongoing_question: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    journey_summary: Option<String>,
    #[diesel(sql_type = BigInt)]
    trace_count: i64,
}

#[derive(QueryableByName, Serialize)]
struct ThreadTrace {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    subtitle: String,
    #[diesel(sql_type = Text)]
    content: String,
    #[diesel(sql_type = Nullable<Timestamp>)]
    finalized_at: Option<chrono::NaiveDateTime>,
}

#[derive(QueryableByName, Serialize)]
struct PreviousMessage {
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Text)]
    content: String,
}

fn error(code: u32, text: &str) -> PpdcError {
    PpdcError::new(code, ErrorType::ApiError, text.to_string())
}

/// One repeatable-read snapshot: edits during preparation cannot mix trace versions.
/// Existing finalized-trace questions retain their old behavior when no draft options are sent.
pub fn prepare(
    owner: Uuid,
    trace_id: Option<Uuid>,
    mentor_id: Option<Uuid>,
    expected_version: Option<i32>,
    thread_id: Option<Uuid>,
    question: &str,
    pool: &DbPool,
) -> Result<Option<PreparedDraftContext>, PpdcError> {
    let has_options = expected_version.is_some() || thread_id.is_some();
    let Some(mentor_id) = mentor_id else {
        return if has_options {
            Err(error(
                400,
                "Draft context options are only supported for mentor questions",
            ))
        } else {
            Ok(None)
        };
    };
    let Some(trace_id) = trace_id else {
        return if has_options {
            Err(error(400, "Draft context requires trace_id"))
        } else {
            Ok(None)
        };
    };
    pool.get()?.build_transaction().repeatable_read().read_only().run(|conn| {
        let draft = diesel::sql_query("SELECT t.id,t.title,t.subtitle,t.content,t.status,t.trace_type,
            t.is_encrypted,t.version_integer,t.continues_trace_id,
            COALESCE(j.status='ACTIVE' AND NOT j.is_encrypted,FALSE) AS journal_available
            FROM traces t LEFT JOIN journals j ON j.id=t.journal_id
            WHERE t.id=$1 AND t.user_id=$2")
            .bind::<SqlUuid,_>(trace_id).bind::<SqlUuid,_>(owner)
            .get_result::<Draft>(conn).optional()?
            .ok_or_else(|| error(404,"Draft trace not found"))?;
        if let Some(expected) = expected_version {
            if draft.version_integer != expected {
                return Err(error(409,"The saved trace version has changed; save or reload before asking the mentor")
                    .with_details(json!({"trace_id":trace_id,"expected_version_integer":expected,
                        "current_version_integer":draft.version_integer})));
            }
        }
        if draft.status != "DRAFT" {
            return if has_options { Err(error(409,"Draft mentor context requires a draft trace")) } else { Ok(None) };
        }
        if expected_version.is_none() {
            return Err(error(400,"expected_version_integer is required for a draft mentor question"));
        }
        if draft.is_encrypted || !draft.journal_available || draft.trace_type != "USER_TRACE" {
            return Err(error(403,"Only unencrypted writing drafts in active journals can be sent to the mentor"));
        }
        if question.trim().is_empty() { return Err(error(400,"A mentor question is required")); }

        // The draft's pending branch is authoritative. An explicit thread must end
        // at its immediate predecessor: no unrelated or sibling branch context.
        let thread = diesel::sql_query("SELECT w.id,w.input_revision,w.compiled_revision,w.title,
            w.ongoing_question,w.journey_summary,w.trace_count FROM writing_threads w
            WHERE w.user_id=$1 AND ($2::uuid IS NULL OR w.id=$2)
            AND (w.pending_trace_id=$3 OR ($2::uuid IS NOT NULL AND w.latest_trace_id=$4))
            ORDER BY (w.pending_trace_id=$3) DESC,w.id LIMIT 1")
            .bind::<SqlUuid,_>(owner).bind::<Nullable<SqlUuid>,_>(thread_id)
            .bind::<SqlUuid,_>(trace_id).bind::<Nullable<SqlUuid>,_>(draft.continues_trace_id)
            .get_result::<Thread>(conn).optional()?;
        if thread_id.is_some() && thread.is_none() {
            return Err(error(404,"Writing thread not found for this draft continuation"));
        }
        let mut input = json!({"draft":draft,"user_question":question,"writing_thread":thread,
            "thread_traces":[],"thread_context_complete":true,"omitted_thread_trace_count":0});
        if serde_json::to_string(&input)?.chars().count() > MAX_CONTEXT_CHARS {
            return Err(error(413,"Draft and question are too large for mentor context"));
        }
        let previous_messages = diesel::sql_query("SELECT CASE WHEN sender_user_id=$1 THEN 'writer' ELSE 'mentor' END AS role,content
            FROM messages WHERE trace_id=$3 AND processing_state='PROCESSED'
            AND ((sender_user_id=$1 AND recipient_user_id=$2) OR (sender_user_id=$2 AND recipient_user_id=$1))
            ORDER BY created_at DESC,id DESC LIMIT 10")
            .bind::<SqlUuid,_>(owner).bind::<SqlUuid,_>(mentor_id).bind::<SqlUuid,_>(trace_id)
            .load::<PreviousMessage>(conn)?;
        let mut previous = Vec::new();
        let mut history_chars = 0;
        let mut used = serde_json::to_string(&input)?.chars().count();
        for message in previous_messages {
            let size = serde_json::to_string(&message)?.chars().count() + 1;
            // Reserve most of the budget for the actual writing, not old replies.
            if history_chars + size > 10_000 || used + size > MAX_CONTEXT_CHARS { break; }
            history_chars += size;
            used += size;
            previous.push(message);
        }
        previous.reverse();
        input["recent_messages_for_draft"] = serde_json::to_value(previous)?;
        let mut included = Vec::<ThreadTrace>::new();
        let mut used = serde_json::to_string(&input)?.chars().count();
        if let Some(thread) = &thread {
            let traces = diesel::sql_query("SELECT t.id,t.title,t.subtitle,t.content,t.finalized_at
                FROM writing_thread_traces m JOIN traces t ON t.id=m.trace_id
                WHERE m.thread_id=$1 AND t.status='FINALIZED' AND NOT t.is_encrypted
                AND t.user_id=$2 ORDER BY m.position DESC LIMIT $3")
                .bind::<SqlUuid,_>(thread.id).bind::<SqlUuid,_>(owner)
                .bind::<BigInt,_>(MAX_THREAD_TRACES).load::<ThreadTrace>(conn)?;
            // Keep the newest complete traces, never silently clip a trace's text.
            for trace in traces {
                let size = serde_json::to_string(&trace)?.chars().count() + 1;
                if used + size > MAX_CONTEXT_CHARS { break; }
                used += size;
                included.push(trace);
            }
        }
        included.reverse();
        let omitted = thread.as_ref().map_or(0,|t| t.trace_count - included.len() as i64).max(0);
        input["thread_traces"] = serde_json::to_value(included)?;
        input["thread_context_complete"] = json!(omitted == 0);
        input["omitted_thread_trace_count"] = json!(omitted);
        Ok(Some(PreparedDraftContext {
            source: DraftMentorSource { trace_id,trace_version_integer:draft.version_integer,
                writing_thread_id:thread.as_ref().map(|t|t.id),
                thread_input_revision:thread.as_ref().map(|t|t.input_revision),
                omitted_thread_trace_count:omitted },
            input,
        }))
    })
}

pub(super) fn save(
    message_id: Uuid,
    prepared: &PreparedDraftContext,
    conn: &mut PgConnection,
) -> Result<(), PpdcError> {
    diesel::sql_query(
        "INSERT INTO mentor_message_contexts(message_id,input_json) VALUES($1,CAST($2 AS jsonb))",
    )
    .bind::<SqlUuid, _>(message_id)
    .bind::<Text, _>(serde_json::to_string(&prepared.input)?)
    .execute(conn)?;
    Ok(())
}

#[derive(QueryableByName)]
struct StoredContext {
    #[diesel(sql_type = Text)]
    input: String,
}

pub fn load(message_id: Uuid, owner: Uuid, pool: &DbPool) -> Result<Value, PpdcError> {
    let row = diesel::sql_query(
        "SELECT c.input_json::text AS input FROM mentor_message_contexts c
        JOIN messages m ON m.id=c.message_id WHERE c.message_id=$1 AND m.sender_user_id=$2",
    )
    .bind::<SqlUuid, _>(message_id)
    .bind::<SqlUuid, _>(owner)
    .get_result::<StoredContext>(&mut pool.get()?)
    .optional()?
    .ok_or_else(|| error(409, "The draft mentor input snapshot is missing"))?;
    Ok(serde_json::from_str(&row.input)?)
}
