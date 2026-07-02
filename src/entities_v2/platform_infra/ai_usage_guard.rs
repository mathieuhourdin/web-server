use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Uuid as SqlUuid};
use serde::Serialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    usage_event::{create_internal_usage_event, UsageEventType},
    user::User,
};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiUsageKind {
    MentorQuestion,
    TarotReading,
    Transcription,
    LandscapeAnalysis,
}

impl AiUsageKind {
    pub fn as_api_value(self) -> &'static str {
        match self {
            AiUsageKind::MentorQuestion => "mentor_question",
            AiUsageKind::TarotReading => "tarot_reading",
            AiUsageKind::Transcription => "transcription",
            AiUsageKind::LandscapeAnalysis => "landscape_analysis",
        }
    }

    pub fn daily_limit(self) -> i64 {
        match self {
            AiUsageKind::MentorQuestion => 20,
            AiUsageKind::TarotReading => 10,
            AiUsageKind::Transcription => 20,
            AiUsageKind::LandscapeAnalysis => 5,
        }
    }
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

pub fn ensure_ai_usage_allowed(
    user: &User,
    session_id: Option<Uuid>,
    kind: AiUsageKind,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    if !user.allows_ai_features() {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "AI features are disabled for this account".to_string(),
        )
        .with_details(serde_json::json!({
            "ai_features_enabled": user.ai_features_enabled,
            "ai_features_enabled_by_admin": user.ai_features_enabled_by_admin
        })));
    }

    let current_count = count_recent_ai_usage(user.id, kind, pool)?;
    let limit = kind.daily_limit();
    if current_count >= limit {
        return Err(PpdcError::new(
            429,
            ErrorType::ApiError,
            format!("Daily AI limit reached for {}", kind.as_api_value()),
        )
        .with_details(serde_json::json!({
            "request_kind": kind.as_api_value(),
            "limit": limit,
            "current_count": current_count,
            "window": "24h"
        })));
    }

    if matches!(kind, AiUsageKind::Transcription) {
        create_internal_usage_event(
            user.id,
            session_id,
            UsageEventType::AiTranscriptionRequested,
            None,
            None,
            pool,
        )?;
    }

    Ok(())
}

fn count_recent_ai_usage(
    user_id: Uuid,
    kind: AiUsageKind,
    pool: &DbPool,
) -> Result<i64, PpdcError> {
    let mut conn = pool.get()?;

    let sql = match kind {
        AiUsageKind::MentorQuestion => {
            r#"
            SELECT COUNT(*)::bigint AS value
            FROM messages m
            INNER JOIN users recipient ON recipient.id = m.recipient_user_id
            WHERE m.sender_user_id = $1
              AND m.message_type = 'QUESTION'
              AND m.created_at >= NOW() - INTERVAL '24 hours'
              AND (
                  recipient.principal_type = 'SERVICE'
                  OR EXISTS (
                      SELECT 1
                      FROM user_roles ur
                      WHERE ur.user_id = recipient.id
                        AND ur.role = 'MENTOR'
                  )
              )
            "#
        }
        AiUsageKind::TarotReading => {
            r#"
            SELECT COUNT(*)::bigint AS value
            FROM messages m
            WHERE m.sender_user_id = $1
              AND m.message_type = 'TAROT_READING_REQUEST'
              AND m.created_at >= NOW() - INTERVAL '24 hours'
            "#
        }
        AiUsageKind::Transcription => {
            r#"
            SELECT COUNT(*)::bigint AS value
            FROM usage_events ue
            WHERE ue.user_id = $1
              AND ue.event_type = 'AI_TRANSCRIPTION_REQUESTED'
              AND ue.occurred_at >= NOW() - INTERVAL '24 hours'
            "#
        }
        AiUsageKind::LandscapeAnalysis => {
            r#"
            SELECT COUNT(*)::bigint AS value
            FROM landscape_analyses la
            WHERE la.user_id = $1
              AND la.landscape_analysis_type IN ('TRACE_INCREMENTAL', 'HLP', 'BIO')
              AND la.created_at >= NOW() - INTERVAL '24 hours'
            "#
        }
    };

    let count = sql_query(sql)
        .bind::<SqlUuid, _>(user_id)
        .get_result::<CountRow>(&mut conn)?
        .value;

    Ok(count)
}
