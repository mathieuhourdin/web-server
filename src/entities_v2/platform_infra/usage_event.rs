use axum::{
    debug_handler,
    extract::{Extension, Json},
};
use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text, Timestamp, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    analysis_summary::AnalysisSummary,
    error::{ErrorType, PpdcError},
    journal::Journal,
    journal_grant::JournalGrant,
    message::{Message, MessageType},
    post::{Post, PostStatus},
    post_grant::PostGrant,
    session::Session,
    trace::Trace,
};
use crate::schema::usage_events;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsageEventType {
    HomeVisited,
    HistoryVisited,
    JournalOpened,
    FollowedJournalOpened,
    PostOpened,
    FeedbackOpened,
    SummaryOpened,
    TraceTimeoutSet,
    TraceTimeoutExtended,
    TraceTimeoutAutoFinalized,
}

impl UsageEventType {
    pub fn to_db(self) -> &'static str {
        match self {
            UsageEventType::HomeVisited => "HOME_VISITED",
            UsageEventType::HistoryVisited => "HISTORY_VISITED",
            UsageEventType::JournalOpened => "JOURNAL_OPENED",
            UsageEventType::FollowedJournalOpened => "FOLLOWED_JOURNAL_OPENED",
            UsageEventType::PostOpened => "POST_OPENED",
            UsageEventType::FeedbackOpened => "FEEDBACK_OPENED",
            UsageEventType::SummaryOpened => "SUMMARY_OPENED",
            UsageEventType::TraceTimeoutSet => "TRACE_TIMEOUT_SET",
            UsageEventType::TraceTimeoutExtended => "TRACE_TIMEOUT_EXTENDED",
            UsageEventType::TraceTimeoutAutoFinalized => "TRACE_TIMEOUT_AUTO_FINALIZED",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, PpdcError> {
        match value {
            "HOME_VISITED" | "home_visited" => Ok(UsageEventType::HomeVisited),
            "HISTORY_VISITED" | "history_visited" => Ok(UsageEventType::HistoryVisited),
            "JOURNAL_OPENED" | "journal_opened" => Ok(UsageEventType::JournalOpened),
            "FOLLOWED_JOURNAL_OPENED" | "followed_journal_opened" => {
                Ok(UsageEventType::FollowedJournalOpened)
            }
            "POST_OPENED" | "post_opened" => Ok(UsageEventType::PostOpened),
            "FEEDBACK_OPENED" | "feedback_opened" => Ok(UsageEventType::FeedbackOpened),
            "SUMMARY_OPENED" | "summary_opened" => Ok(UsageEventType::SummaryOpened),
            "TRACE_TIMEOUT_SET" | "trace_timeout_set" => Ok(UsageEventType::TraceTimeoutSet),
            "TRACE_TIMEOUT_EXTENDED" | "trace_timeout_extended" => {
                Ok(UsageEventType::TraceTimeoutExtended)
            }
            "TRACE_TIMEOUT_AUTO_FINALIZED" | "trace_timeout_auto_finalized" => {
                Ok(UsageEventType::TraceTimeoutAutoFinalized)
            }
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid usage event type: {}", value),
            )),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct UsageEvent {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub event_type: UsageEventType,
    pub resource_id: Option<Uuid>,
    pub context_json: Option<Value>,
    pub occurred_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NewUsageEventDto {
    pub event_type: UsageEventType,
    pub resource_id: Option<Uuid>,
    pub context_json: Option<Value>,
}

#[derive(Debug, Clone)]
struct NewUsageEvent {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub event_type: UsageEventType,
    pub resource_id: Option<Uuid>,
    pub context_json: Option<Value>,
    pub occurred_at: NaiveDateTime,
}

type UsageEventTuple = (
    Uuid,
    Uuid,
    Option<Uuid>,
    String,
    Option<Uuid>,
    Option<String>,
    NaiveDateTime,
    NaiveDateTime,
);

impl TryFrom<UsageEventTuple> for UsageEvent {
    type Error = PpdcError;

    fn try_from(row: UsageEventTuple) -> Result<Self, Self::Error> {
        let (
            id,
            user_id,
            session_id,
            event_type,
            resource_id,
            context_json,
            occurred_at,
            created_at,
        ) = row;
        Ok(UsageEvent {
            id,
            user_id,
            session_id,
            event_type: UsageEventType::from_db(&event_type)?,
            resource_id,
            context_json: context_json.and_then(|json| serde_json::from_str::<Value>(&json).ok()),
            occurred_at,
            created_at,
        })
    }
}

impl NewUsageEvent {
    fn new(payload: NewUsageEventDto, user_id: Uuid, session_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            session_id,
            event_type: payload.event_type,
            resource_id: payload.resource_id,
            context_json: payload.context_json,
            occurred_at: Utc::now().naive_utc(),
        }
    }

    fn create(self, pool: &DbPool) -> Result<UsageEvent, PpdcError> {
        let mut conn = pool
            .get()?;

        sql_query(
            r#"
            INSERT INTO usage_events (
                id,
                user_id,
                session_id,
                event_type,
                resource_id,
                context_json,
                occurred_at,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, CAST($6 AS jsonb), $7, $8)
            "#,
        )
        .bind::<SqlUuid, _>(self.id)
        .bind::<SqlUuid, _>(self.user_id)
        .bind::<Nullable<SqlUuid>, _>(self.session_id)
        .bind::<Text, _>(self.event_type.to_db())
        .bind::<Nullable<SqlUuid>, _>(self.resource_id)
        .bind::<Nullable<Text>, _>(self.context_json.as_ref().map(|value| value.to_string()))
        .bind::<Timestamp, _>(self.occurred_at)
        .bind::<Timestamp, _>(self.occurred_at)
        .execute(&mut conn)?;

        UsageEvent::find(self.id, pool)
    }
}

impl UsageEvent {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<UsageEvent, PpdcError> {
        let mut conn = pool
            .get()?;
        let row = usage_events::table
            .filter(usage_events::id.eq(id))
            .select((
                usage_events::id,
                usage_events::user_id,
                usage_events::session_id,
                usage_events::event_type,
                usage_events::resource_id,
                diesel::dsl::sql::<Nullable<Text>>("context_json::text"),
                usage_events::occurred_at,
                usage_events::created_at,
            ))
            .first::<UsageEventTuple>(&mut conn)
            .optional()?;

        row.map(TryInto::try_into).transpose()?.ok_or_else(|| {
            PpdcError::new(404, ErrorType::ApiError, "Usage event not found".to_string())
        })
    }
}

pub fn create_usage_event(
    user_id: Uuid,
    session_id: Option<Uuid>,
    event_type: UsageEventType,
    resource_id: Option<Uuid>,
    context_json: Option<Value>,
    pool: &DbPool,
) -> Result<UsageEvent, PpdcError> {
    let payload = NewUsageEventDto {
        event_type,
        resource_id,
        context_json,
    };
    validate_usage_event_access(event_type, resource_id, user_id, pool)?;
    NewUsageEvent::new(payload, user_id, session_id).create(pool)
}

fn validate_usage_event_access(
    event_type: UsageEventType,
    resource_id: Option<Uuid>,
    user_id: Uuid,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    match event_type {
        UsageEventType::HomeVisited | UsageEventType::HistoryVisited => {
            if resource_id.is_some() {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "resource_id must be null for this event type".to_string(),
                ));
            }
        }
        UsageEventType::JournalOpened | UsageEventType::FollowedJournalOpened => {
            let resource_id = resource_id.ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "resource_id is required for this event type".to_string(),
                )
            })?;
            let journal = Journal::find_full(resource_id, pool)?;
            if !JournalGrant::user_can_read_journal(&journal, user_id, pool)? {
                return Err(PpdcError::unauthorized());
            }
        }
        UsageEventType::PostOpened => {
            let resource_id = resource_id.ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "resource_id is required for this event type".to_string(),
                )
            })?;
            let post = Post::find_full(resource_id, pool)?;
            if post.user_id != user_id && post.status != PostStatus::Published {
                return Err(PpdcError::unauthorized());
            }
            if !PostGrant::user_can_read_post(&post, user_id, pool)? {
                return Err(PpdcError::unauthorized());
            }
        }
        UsageEventType::FeedbackOpened => {
            let resource_id = resource_id.ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "resource_id is required for this event type".to_string(),
                )
            })?;
            let message = Message::find(resource_id, pool)?;
            if message.message_type != MessageType::MentorFeedback
                || message.recipient_user_id != user_id
            {
                return Err(PpdcError::unauthorized());
            }
        }
        UsageEventType::SummaryOpened => {
            let resource_id = resource_id.ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "resource_id is required for this event type".to_string(),
                )
            })?;
            let summary = AnalysisSummary::find(resource_id, pool)?;
            if summary.user_id != user_id {
                return Err(PpdcError::unauthorized());
            }
        }
        UsageEventType::TraceTimeoutSet
        | UsageEventType::TraceTimeoutExtended
        | UsageEventType::TraceTimeoutAutoFinalized => {
            let resource_id = resource_id.ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "resource_id is required for this event type".to_string(),
                )
            })?;
            let trace = Trace::find_full_trace(resource_id, pool)?;
            if trace.user_id != user_id {
                return Err(PpdcError::unauthorized());
            }
        }
    }

    Ok(())
}

#[debug_handler]
pub async fn post_usage_event_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewUsageEventDto>,
) -> Result<Json<UsageEvent>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let usage_event = create_usage_event(
        user_id,
        Some(session.id),
        payload.event_type,
        payload.resource_id,
        payload.context_json,
        &pool,
    )?;
    Ok(Json(usage_event))
}
