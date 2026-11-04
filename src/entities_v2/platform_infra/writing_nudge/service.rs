use chrono::{DateTime, Duration, TimeZone, Utc};
use chrono_tz::Tz;
use diesel::{
    prelude::*,
    sql_types::{Bool, Integer, Nullable, Text, Timestamp, Timestamptz, Uuid as SqlUuid},
};
use pulldown_cmark::{Event, Parser, TagEnd};
use uuid::Uuid;

use super::model::{CurrentNudge, NudgeEvent, NudgeType, TracePreview, WritingNudge};
use crate::{
    db::DbPool,
    entities_v2::error::{ErrorType, PpdcError},
};

#[derive(QueryableByName)]
struct Owner {
    #[diesel(sql_type=Text)]
    timezone: String,
    #[diesel(sql_type=Text)]
    principal_type: String,
}

#[derive(QueryableByName)]
struct Activity {
    #[diesel(sql_type=Bool)]
    has_writing: bool,
    #[diesel(sql_type=Nullable<Timestamp>)]
    last_written_at: Option<chrono::NaiveDateTime>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    last_activity_at: Option<chrono::NaiveDateTime>,
}

#[derive(QueryableByName)]
struct State {
    #[diesel(sql_type=Text)]
    nudge_type: String,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    suggestion_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    trace_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Timestamptz>)]
    issued_at: Option<DateTime<Utc>>,
    #[diesel(sql_type=Nullable<Timestamptz>)]
    expires_at: Option<DateTime<Utc>>,
    #[diesel(sql_type=Nullable<Timestamptz>)]
    first_shown_at: Option<DateTime<Utc>>,
    #[diesel(sql_type=Nullable<Text>)]
    terminal_event: Option<String>,
    #[diesel(sql_type=Nullable<Timestamptz>)]
    last_shown_at: Option<DateTime<Utc>>,
    #[diesel(sql_type=Nullable<Timestamptz>)]
    last_opened_at: Option<DateTime<Utc>>,
    #[diesel(sql_type=Nullable<Timestamptz>)]
    last_dismissed_at: Option<DateTime<Utc>>,
    #[diesel(sql_type=Nullable<Timestamptz>)]
    dismissed_until: Option<DateTime<Utc>>,
    #[diesel(sql_type=Integer)]
    dismissal_count: i32,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    last_suggested_trace_id: Option<Uuid>,
}

impl State {
    fn kind(&self) -> Result<NudgeType, PpdcError> {
        NudgeType::from_db(&self.nudge_type).ok_or_else(|| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "Invalid writing nudge state".into(),
            )
        })
    }

    fn live(&self, now: DateTime<Utc>) -> bool {
        let terminal = self.terminal_event.as_deref().map(NudgeEvent::from_db);
        self.suggestion_id.is_some()
            && self.expires_at.is_some_and(|at| at > now)
            && matches!(terminal, None | Some(Some(NudgeEvent::Shown)))
    }

    fn response(&self, trace: Option<TracePreview>) -> Result<WritingNudge, PpdcError> {
        let invalid = || {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "Incomplete writing nudge state".into(),
            )
        };
        Ok(WritingNudge {
            id: self.suggestion_id.ok_or_else(invalid)?,
            kind: self.kind()?,
            issued_at: self.issued_at.ok_or_else(invalid)?,
            expires_at: self.expires_at.ok_or_else(invalid)?,
            trace,
        })
    }
}

#[derive(QueryableByName)]
struct Candidate {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    journal_id: Option<Uuid>,
    #[diesel(sql_type=Text)]
    title: String,
    #[diesel(sql_type=Text)]
    content: String,
    #[diesel(sql_type=Nullable<Timestamp>)]
    finalized_at: Option<chrono::NaiveDateTime>,
    #[diesel(sql_type=Timestamp)]
    updated_at: chrono::NaiveDateTime,
}

impl Candidate {
    fn preview(self) -> TracePreview {
        TracePreview {
            id: self.id,
            journal_id: self.journal_id,
            title: self.title,
            excerpt: excerpt(&self.content),
            finalized_at: self.finalized_at.map(|at| at.and_utc()),
            updated_at: self.updated_at.and_utc(),
        }
    }
}

fn lock_owner(user: Uuid, conn: &mut PgConnection) -> Result<Owner, PpdcError> {
    // Serializes all three types and all devices, without adding fields to User.
    Ok(
        diesel::sql_query("SELECT timezone,principal_type FROM users WHERE id=$1 FOR UPDATE")
            .bind::<SqlUuid, _>(user)
            .get_result(conn)?,
    )
}

fn states(user: Uuid, conn: &mut PgConnection) -> Result<Vec<State>, PpdcError> {
    Ok(diesel::sql_query("SELECT * FROM user_writing_nudge_states WHERE user_id=$1 ORDER BY issued_at DESC NULLS LAST")
        .bind::<SqlUuid,_>(user).load(conn)?)
}

fn activity(user: Uuid, conn: &mut PgConnection) -> Result<Activity, PpdcError> {
    // updated_at is a conservative edit-activity proxy: metadata changes may
    // postpone a nudge, rather than nagging an active writer. No content is loaded.
    Ok(diesel::sql_query(
        "SELECT COUNT(*)>0 AS has_writing,
         MAX(CASE WHEN NOT is_historical_import THEN GREATEST(updated_at,finalized_at) END) AS last_written_at,
         MAX(GREATEST(updated_at,finalized_at,created_at)) AS last_activity_at
         FROM traces WHERE user_id=$1 AND NOT is_blank
             AND trace_type IN ('USER_TRACE','WORKSPACE_TRACE')"
    ).bind::<SqlUuid,_>(user).get_result(conn)?)
}

fn reset_escalation(
    user: Uuid,
    activity: &Activity,
    conn: &mut PgConnection,
) -> Result<(), PpdcError> {
    if let Some(written) = activity.last_written_at {
        // Keep the already-promised quiet period; reset only future escalation.
        diesel::sql_query(
            "UPDATE user_writing_nudge_states SET dismissal_count=0,updated_at=NOW()
            WHERE user_id=$1 AND dismissal_count>0 AND last_dismissed_at < $2",
        )
        .bind::<SqlUuid, _>(user)
        .bind::<Timestamptz, _>(written.and_utc())
        .execute(conn)?;
    }
    Ok(())
}

fn eligible(kind: NudgeType, activity: &Activity, now: DateTime<Utc>) -> bool {
    match kind {
        NudgeType::FirstTrace => !activity.has_writing,
        NudgeType::ResumeDraft => true,
        NudgeType::WritingMemory => {
            activity.has_writing
                && activity
                    .last_activity_at
                    .is_some_and(|at| at.and_utc() <= now - Duration::days(14))
        }
    }
}

fn candidates(
    user: Uuid,
    kind: NudgeType,
    only: Option<Uuid>,
    now: DateTime<Utc>,
    conn: &mut PgConnection,
) -> Result<Vec<Candidate>, PpdcError> {
    Ok(diesel::sql_query(
        "SELECT t.id,t.journal_id,t.title,LEFT(t.content,4096) AS content,t.finalized_at,t.updated_at
         FROM traces t LEFT JOIN journals j ON j.id=t.journal_id
         WHERE t.user_id=$1 AND NOT t.is_blank AND NOT t.is_encrypted
           AND t.trace_type IN ('USER_TRACE','WORKSPACE_TRACE')
           AND (t.journal_id IS NULL OR (j.user_id=$1 AND j.status='ACTIVE' AND NOT j.is_encrypted))
           AND ($3::uuid IS NULL OR t.id=$3)
           AND (($4='RESUME_DRAFT' AND t.status='DRAFT' AND t.updated_at <= $2-INTERVAL '3 days'
                 AND (t.timeout_at IS NULL OR t.timeout_at>($2 AT TIME ZONE 'UTC')))
             OR ($4='WRITING_MEMORY' AND t.status='FINALIZED' AND t.finalized_at <= $2))
         ORDER BY CASE WHEN $4='WRITING_MEMORY' AND t.finalized_at <= $2-INTERVAL '30 days' THEN 0 ELSE 1 END,
             CASE WHEN $4='WRITING_MEMORY' THEN t.finalized_at ELSE t.updated_at END DESC,t.id
         LIMIT 2"
    ).bind::<SqlUuid,_>(user).bind::<Timestamp,_>(now.naive_utc())
        .bind::<Nullable<SqlUuid>,_>(only).bind::<Text,_>(kind.to_db()).load(conn)?)
}

fn current_preview(
    user: Uuid,
    state: &State,
    activity: &Activity,
    now: DateTime<Utc>,
    conn: &mut PgConnection,
) -> Result<Option<Option<TracePreview>>, PpdcError> {
    let kind = state.kind()?;
    if !eligible(kind, activity, now) {
        return Ok(None);
    }
    if kind == NudgeType::FirstTrace {
        return Ok(Some(None));
    }
    let Some(trace) = state.trace_id else {
        return Ok(None);
    };
    Ok(candidates(user, kind, Some(trace), now, conn)?
        .into_iter()
        .next()
        .map(|c| Some(c.preview())))
}

fn cooling_down(states: &[State], now: DateTime<Utc>, tz: Tz) -> bool {
    states.iter().any(|state| {
        state.last_shown_at.is_some_and(|at| {
            at.with_timezone(&tz).date_naive() == now.with_timezone(&tz).date_naive()
                || at + Duration::days(3) > now
        }) || state
            .last_opened_at
            .is_some_and(|at| at + Duration::days(3) > now)
            || state.dismissed_until.is_some_and(|at| at > now)
    })
}

fn next_midnight(now: DateTime<Utc>, tz: Tz) -> Result<DateTime<Utc>, PpdcError> {
    let next = now
        .with_timezone(&tz)
        .date_naive()
        .succ_opt()
        .and_then(|day| day.and_hms_opt(0, 0, 0))
        .ok_or_else(|| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "Invalid nudge expiration".into(),
            )
        })?;
    // Handle DST midnight gaps, ambiguous midnights and skipped civil dates.
    for minute in 0..=2880 {
        if let Some(at) = tz
            .from_local_datetime(&(next + Duration::minutes(minute)))
            .earliest()
        {
            return Ok(at.with_timezone(&Utc));
        }
    }
    Err(PpdcError::new(
        500,
        ErrorType::InternalError,
        "Invalid nudge timezone boundary".into(),
    ))
}

pub(super) fn current(
    user: Uuid,
    now: DateTime<Utc>,
    pool: &DbPool,
) -> Result<CurrentNudge, PpdcError> {
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        let owner = lock_owner(user, conn)?;
        if owner.principal_type != "HUMAN" { return Ok(CurrentNudge { nudge: None }); }
        diesel::sql_query("INSERT INTO user_writing_nudge_states(user_id,nudge_type)
            VALUES($1,'RESUME_DRAFT'),($1,'WRITING_MEMORY'),($1,'FIRST_TRACE') ON CONFLICT DO NOTHING")
            .bind::<SqlUuid,_>(user).execute(conn)?;
        let activity = activity(user, conn)?;
        reset_escalation(user, &activity, conn)?;
        let states = states(user, conn)?;
        let tz = owner.timezone.parse::<Tz>().unwrap_or(chrono_tz::UTC);
        for state in states.iter().filter(|s| s.live(now)) {
            if let Some(preview) = current_preview(user, state, &activity, now, conn)? {
                return Ok(CurrentNudge { nudge: Some(state.response(preview)?) });
            }
            diesel::sql_query("UPDATE user_writing_nudge_states SET expires_at=$2,updated_at=$2
                WHERE user_id=$1 AND suggestion_id=$3")
                .bind::<SqlUuid,_>(user).bind::<Timestamptz,_>(now)
                .bind::<Nullable<SqlUuid>,_>(state.suggestion_id).execute(conn)?;
        }
        if cooling_down(&states, now, tz) { return Ok(CurrentNudge { nudge: None }); }
        for kind in NudgeType::ALL {
            if !eligible(kind, &activity, now) { continue; }
            let previous = states.iter().find(|s| s.nudge_type==kind.to_db()).expect("initialized nudge state");
            let trace = if kind == NudgeType::FirstTrace { None } else {
                let mut choices = candidates(user, kind, None, now, conn)?;
                let choice = choices.iter().position(|c| Some(c.id)!=previous.last_suggested_trace_id)
                    .or_else(|| (choices.len()==1 && previous.last_shown_at
                        .is_none_or(|at| at+Duration::days(30)<=now)).then_some(0));
                let Some(index) = choice else { continue; };
                Some(choices.remove(index).preview())
            };
            let id = Uuid::new_v4();
            let expires = next_midnight(now, tz)?;
            // Superseded IDs become inert, even if a device reports a late event.
            diesel::sql_query("UPDATE user_writing_nudge_states SET expires_at=LEAST(expires_at,$2),updated_at=$2
                WHERE user_id=$1 AND suggestion_id IS NOT NULL")
                .bind::<SqlUuid,_>(user).bind::<Timestamptz,_>(now).execute(conn)?;
            diesel::sql_query("UPDATE user_writing_nudge_states SET suggestion_id=$3,trace_id=$4,
                issued_at=$5,expires_at=$6,first_shown_at=NULL,terminal_event=NULL,updated_at=$5
                WHERE user_id=$1 AND nudge_type=$2")
                .bind::<SqlUuid,_>(user).bind::<Text,_>(kind.to_db()).bind::<SqlUuid,_>(id)
                .bind::<Nullable<SqlUuid>,_>(trace.as_ref().map(|t| t.id))
                .bind::<Timestamptz,_>(now).bind::<Timestamptz,_>(expires).execute(conn)?;
            return Ok(CurrentNudge { nudge: Some(WritingNudge { id, kind, issued_at: now, expires_at: expires, trace }) });
        }
        Ok(CurrentNudge { nudge: None })
    })
}

fn dismissal_days(count: i32) -> i64 {
    match count {
        0 | 1 => 7,
        2 => 14,
        _ => 30,
    }
}

pub(super) fn record_event(
    user: Uuid,
    id: Uuid,
    event: NudgeEvent,
    now: DateTime<Utc>,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut conn = pool.get()?;
    conn.transaction(|conn| {
        let owner = lock_owner(user, conn)?;
        if owner.principal_type != "HUMAN" { return Ok(()); }
        let states = states(user, conn)?;
        let Some(state) = states.iter().find(|s| s.suggestion_id==Some(id) && s.live(now)) else { return Ok(()); };
        if event==NudgeEvent::Shown && state.first_shown_at.is_some() { return Ok(()); }
        let tz = owner.timezone.parse::<Tz>().unwrap_or(chrono_tz::UTC);
        if state.first_shown_at.is_none() && states.iter().any(|s| s.suggestion_id!=Some(id)
            && s.last_shown_at.is_some_and(|at| at.with_timezone(&tz).date_naive()==now.with_timezone(&tz).date_naive())) {
            return Ok(());
        }
        let activity = activity(user, conn)?;
        let previous_count = if activity.last_written_at.is_some_and(|at|
            state.last_dismissed_at.is_some_and(|dismissed| at.and_utc()>dismissed)) {
            0
        } else { state.dismissal_count };
        let count = if event==NudgeEvent::Dismissed { previous_count.saturating_add(1) } else { previous_count };
        diesel::sql_query("UPDATE user_writing_nudge_states SET
            first_shown_at=COALESCE(first_shown_at,$3),
            last_shown_at=CASE WHEN first_shown_at IS NULL THEN $3 ELSE last_shown_at END,
            last_suggested_trace_id=CASE WHEN first_shown_at IS NULL THEN trace_id ELSE last_suggested_trace_id END,
            terminal_event=CASE WHEN $4='SHOWN' THEN NULL ELSE $4 END,
            last_opened_at=CASE WHEN $4='OPENED' THEN $3 ELSE last_opened_at END,
            last_dismissed_at=CASE WHEN $4='DISMISSED' THEN $3 ELSE last_dismissed_at END,
            dismissed_until=CASE WHEN $4='DISMISSED' THEN $5 ELSE dismissed_until END,
            dismissal_count=$6,updated_at=$3
            WHERE user_id=$1 AND suggestion_id=$2")
            .bind::<SqlUuid,_>(user).bind::<SqlUuid,_>(id).bind::<Timestamptz,_>(now)
            .bind::<Text,_>(event.to_db()).bind::<Timestamptz,_>(now+Duration::days(dismissal_days(count)))
            .bind::<Integer,_>(count).execute(conn)?;
        Ok(())
    })
}

fn excerpt(content: &str) -> String {
    let mut text = String::new();
    for event in Parser::new(content) {
        match event {
            Event::Text(value) | Event::Code(value) => text.push_str(&value),
            Event::SoftBreak | Event::HardBreak | Event::Rule => text.push(' '),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::BlockQuote(_)
                | TagEnd::CodeBlock
                | TagEnd::TableCell,
            ) => text.push(' '),
            _ => (),
        }
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > 240 {
        format!("{}…", text.chars().take(239).collect::<String>())
    } else {
        text
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
