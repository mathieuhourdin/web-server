use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Bool, Nullable, Text, Timestamp, Timestamptz, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::post::PostStatus;
use crate::entities_v2::post_grant::PostGrant;
use crate::schema::{posts, traces};

pub use super::enums::{TraceSharingSensitivity, TraceStatus, TraceType};

#[derive(Deserialize)]
pub struct NewTraceDto {
    #[serde(default)]
    pub content: String,
    pub journal_id: Uuid,
    #[serde(default)]
    pub derived_from_trace_id: Option<Uuid>,
    #[serde(default)]
    pub interaction_date: Option<NaiveDateTime>,
    #[serde(default, alias = "is_ecrypted")]
    pub is_encrypted: Option<bool>,
    #[serde(default)]
    pub encryption_metadata: Option<Value>,
    #[serde(default)]
    pub image_asset_id: Option<Uuid>,
    #[serde(default)]
    pub sharing_sensitivity: Option<TraceSharingSensitivity>,
    #[serde(default)]
    pub timeout_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct UpdateTraceDto {
    pub content: Option<String>,
    pub derived_from_trace_id: Option<Option<Uuid>>,
    pub interaction_date: Option<NaiveDateTime>,
    pub status: Option<TraceStatus>,
    pub image_asset_id: Option<Option<Uuid>>,
    pub sharing_sensitivity: Option<TraceSharingSensitivity>,
    pub timeout_at: Option<Option<DateTime<Utc>>>,
    pub publish_default_post: Option<bool>,
}

#[derive(Deserialize)]
pub struct PatchTraceDto {
    pub content: Option<String>,
    pub derived_from_trace_id: Option<Option<Uuid>>,
    pub interaction_date: Option<NaiveDateTime>,
    pub image_asset_id: Option<Option<Uuid>>,
    pub sharing_sensitivity: Option<TraceSharingSensitivity>,
    pub timeout_at: Option<Option<DateTime<Utc>>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Trace {
    pub id: Uuid,
    pub derived_from_trace_id: Option<Uuid>,
    pub title: String,
    pub subtitle: String,
    pub interaction_date: NaiveDateTime,
    pub content: String,
    pub is_encrypted: bool,
    pub encryption_metadata: Option<Value>,
    pub image_asset_id: Option<Uuid>,
    pub sharing_sensitivity: TraceSharingSensitivity,
    pub timeout_start_at: Option<DateTime<Utc>>,
    pub timeout_at: Option<DateTime<Utc>>,
    pub journal_id: Option<Uuid>,
    pub user_id: Uuid,
    pub trace_type: TraceType,
    pub status: TraceStatus,
    pub start_writing_at: NaiveDateTime,
    pub finalized_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JournalTraceView {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub title: String,
    pub content: String,
    pub image_asset_id: Option<Uuid>,
    pub seen: bool,
    pub last_seen_at: Option<NaiveDateTime>,
    pub interaction_date: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(QueryableByName, Debug)]
pub(crate) struct TraceRow {
    #[diesel(sql_type = SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    pub derived_from_trace_id: Option<Uuid>,
    #[diesel(sql_type = Text)]
    pub title: String,
    #[diesel(sql_type = Text)]
    pub subtitle: String,
    #[diesel(sql_type = Timestamp)]
    pub interaction_date: NaiveDateTime,
    #[diesel(sql_type = Text)]
    pub content: String,
    #[diesel(sql_type = Bool)]
    pub is_encrypted: bool,
    #[diesel(sql_type = Nullable<Text>)]
    pub encryption_metadata: Option<String>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    pub image_asset_id: Option<Uuid>,
    #[diesel(sql_type = Text)]
    pub sharing_sensitivity: String,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    pub timeout_start_at: Option<DateTime<Utc>>,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    pub timeout_at: Option<DateTime<Utc>>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    pub journal_id: Option<Uuid>,
    #[diesel(sql_type = SqlUuid)]
    pub user_id: Uuid,
    #[diesel(sql_type = Text)]
    pub trace_type: String,
    #[diesel(sql_type = Text)]
    pub status: String,
    #[diesel(sql_type = Timestamp)]
    pub start_writing_at: NaiveDateTime,
    #[diesel(sql_type = Nullable<Timestamp>)]
    pub finalized_at: Option<NaiveDateTime>,
    #[diesel(sql_type = Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    pub updated_at: NaiveDateTime,
}

#[derive(QueryableByName, Debug)]
struct OptionalTraceVersionIdRow {
    #[diesel(sql_type = Nullable<SqlUuid>)]
    pub id: Option<Uuid>,
}

impl From<TraceRow> for Trace {
    fn from(row: TraceRow) -> Self {
        Trace {
            id: row.id,
            derived_from_trace_id: row.derived_from_trace_id,
            title: row.title,
            subtitle: row.subtitle,
            interaction_date: row.interaction_date,
            content: row.content,
            is_encrypted: row.is_encrypted,
            encryption_metadata: row
                .encryption_metadata
                .and_then(|json| serde_json::from_str::<Value>(&json).ok()),
            image_asset_id: row.image_asset_id,
            sharing_sensitivity: TraceSharingSensitivity::from_db(&row.sharing_sensitivity),
            timeout_start_at: row.timeout_start_at,
            timeout_at: row.timeout_at,
            journal_id: row.journal_id,
            user_id: row.user_id,
            trace_type: TraceType::from_db(&row.trace_type),
            status: TraceStatus::from_db(&row.status),
            start_writing_at: row.start_writing_at,
            finalized_at: row.finalized_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl Trace {
    pub fn current_version_id(trace_id: Uuid, pool: &DbPool) -> Result<Option<Uuid>, PpdcError> {
        let mut conn = pool.get()?;
        let row = diesel::sql_query(
            "SELECT current_version_id AS id
             FROM traces
             WHERE id = $1",
        )
        .bind::<SqlUuid, _>(trace_id)
        .get_result::<OptionalTraceVersionIdRow>(&mut conn)?;
        Ok(row.id)
    }

    pub fn effective_datetime(&self) -> NaiveDateTime {
        self.interaction_date
    }

    pub fn find_draft_for_journal(
        journal_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Trace>, PpdcError> {
        let mut conn = pool.get()?;

        let row = diesel::sql_query(
            "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE journal_id = $1
               AND trace_type = 'USER_TRACE'
               AND status = 'DRAFT'
             ORDER BY start_writing_at DESC, created_at DESC
             LIMIT 1",
        )
        .bind::<SqlUuid, _>(journal_id)
        .get_result::<TraceRow>(&mut conn)
        .optional()?;

        Ok(row.map(Trace::from))
    }

    fn find_first_user_trace_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Trace>, PpdcError> {
        let mut conn = pool.get()?;

        let row = diesel::sql_query(
            "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE user_id = $1
               AND trace_type = 'USER_TRACE'
             ORDER BY interaction_date ASC NULLS LAST, created_at ASC
             LIMIT 1",
        )
        .bind::<SqlUuid, _>(user_id)
        .get_result::<TraceRow>(&mut conn)
        .optional()?;

        Ok(row.map(Trace::from))
    }

    pub fn get_most_recent_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Trace>, PpdcError> {
        let mut conn = pool.get()?;

        let row = diesel::sql_query(
            "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE user_id = $1
             ORDER BY interaction_date DESC NULLS LAST, created_at DESC
             LIMIT 1",
        )
        .bind::<SqlUuid, _>(user_id)
        .get_result::<TraceRow>(&mut conn)
        .optional()?;

        Ok(row.map(Trace::from))
    }

    pub fn get_between(
        user_id: Uuid,
        start_trace_id: Uuid,
        end_trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Trace>, PpdcError> {
        let start_trace = Trace::find_full_trace(start_trace_id, pool)?;
        let end_trace = Trace::find_full_trace(end_trace_id, pool)?;

        let from = start_trace.interaction_date;
        let to = end_trace.interaction_date;

        let mut conn = pool.get()?;

        let rows = diesel::sql_query(
            "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE user_id = $1
               AND interaction_date BETWEEN $2 AND $3
             ORDER BY interaction_date ASC, created_at ASC",
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Timestamp, _>(from)
        .bind::<Timestamp, _>(to)
        .load::<TraceRow>(&mut conn)?;

        Ok(rows.into_iter().map(Trace::from).collect())
    }

    pub fn get_before(
        user_id: Uuid,
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Trace>, PpdcError> {
        let trace = Trace::find_full_trace(trace_id, pool)?;
        let until = trace.interaction_date;

        let mut conn = pool.get()?;

        let rows = diesel::sql_query(
            "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE user_id = $1
               AND interaction_date <= $2
             ORDER BY interaction_date ASC, created_at ASC",
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Timestamp, _>(until)
        .load::<TraceRow>(&mut conn)?;

        Ok(rows.into_iter().map(Trace::from).collect())
    }

    pub fn get_first(user_id: Uuid, pool: &DbPool) -> Result<Option<Trace>, PpdcError> {
        let mut conn = pool.get()?;

        let latest_hlp = diesel::sql_query(
            "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE user_id = $1
               AND trace_type = 'HIGH_LEVEL_PROJECTS_DEFINITION'
             ORDER BY interaction_date DESC NULLS LAST, created_at DESC
             LIMIT 1",
        )
        .bind::<SqlUuid, _>(user_id)
        .get_result::<TraceRow>(&mut conn)
        .optional()?;

        if let Some(row) = latest_hlp {
            return Ok(Some(row.into()));
        }

        let latest_bio = diesel::sql_query(
            "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE user_id = $1
               AND trace_type = 'BIO_TRACE'
             ORDER BY interaction_date DESC NULLS LAST, created_at DESC
             LIMIT 1",
        )
        .bind::<SqlUuid, _>(user_id)
        .get_result::<TraceRow>(&mut conn)
        .optional()?;

        if let Some(row) = latest_bio {
            return Ok(Some(row.into()));
        }

        Trace::find_first_user_trace_for_user(user_id, pool)
    }

    pub fn get_next(
        user_id: Uuid,
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Trace>, PpdcError> {
        let trace = Trace::find_full_trace(trace_id, pool)?;

        if trace.trace_type == TraceType::HighLevelProjectsDefinition {
            let mut conn = pool.get()?;

            let latest_bio = diesel::sql_query(
                "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
                 FROM traces
                 WHERE user_id = $1
                   AND trace_type = 'BIO_TRACE'
                 ORDER BY interaction_date DESC NULLS LAST, created_at DESC
                 LIMIT 1",
            )
            .bind::<SqlUuid, _>(user_id)
            .get_result::<TraceRow>(&mut conn)
            .optional()?;

            if let Some(row) = latest_bio {
                return Ok(Some(row.into()));
            }

            return Trace::find_first_user_trace_for_user(user_id, pool);
        }

        if trace.trace_type == TraceType::BioTrace {
            return Trace::find_first_user_trace_for_user(user_id, pool);
        }

        let reference_ts = trace.interaction_date;
        let mut conn = pool.get()?;

        let next = diesel::sql_query(
            "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE user_id = $1
               AND trace_type = 'USER_TRACE'
               AND interaction_date > $2
             ORDER BY interaction_date ASC, created_at ASC
             LIMIT 1",
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Timestamp, _>(reference_ts)
        .get_result::<TraceRow>(&mut conn)
        .optional()?;

        Ok(next.map(Trace::from))
    }

    pub fn get_all_for_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<Trace>, PpdcError> {
        let (items, _) = Self::get_all_for_user_paginated(user_id, 0, i64::MAX / 4, pool)?;
        Ok(items)
    }

    pub fn get_all_for_user_paginated(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Trace>, i64), PpdcError> {
        let mut conn = pool.get()?;

        let total = diesel::sql_query(
            "SELECT COUNT(*)::bigint AS count
             FROM traces
             WHERE user_id = $1",
        )
        .bind::<SqlUuid, _>(user_id)
        .get_result::<CountRow>(&mut conn)?
        .count;

        let rows = diesel::sql_query(
            "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE user_id = $1
             ORDER BY interaction_date DESC NULLS LAST, created_at DESC
             OFFSET $2
             LIMIT $3",
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<diesel::sql_types::BigInt, _>(offset)
        .bind::<diesel::sql_types::BigInt, _>(limit)
        .load::<TraceRow>(&mut conn)?;

        Ok((rows.into_iter().map(Trace::from).collect(), total))
    }

    pub fn get_all_for_journal(journal_id: Uuid, pool: &DbPool) -> Result<Vec<Trace>, PpdcError> {
        let (items, _) = Self::get_for_journal_paginated(journal_id, 0, i64::MAX / 4, None, pool)?;
        Ok(items)
    }

    pub fn get_for_journal_paginated(
        journal_id: Uuid,
        offset: i64,
        limit: i64,
        sharing_sensitivity: Option<TraceSharingSensitivity>,
        pool: &DbPool,
    ) -> Result<(Vec<Trace>, i64), PpdcError> {
        let mut conn = pool.get()?;

        let filter_string;
        if let Some(sharing_sensitivity_filter) = sharing_sensitivity {
            filter_string = format!(
                "AND status = 'FINALIZED' AND sharing_sensitivity = '{}'",
                sharing_sensitivity_filter.to_db()
            );
        } else {
            filter_string = String::from("AND status = 'FINALIZED'");
        }

        let total = diesel::sql_query(format!(
            "SELECT COUNT(*)::bigint AS count
                 FROM traces
                 WHERE journal_id = $1
                   {}",
            filter_string
        ))
        .bind::<SqlUuid, _>(journal_id)
        .get_result::<CountRow>(&mut conn)?
        .count;

        let rows = diesel::sql_query(
            format!(
                "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
                 FROM traces
                 WHERE journal_id = $1
                   {} 
                 ORDER BY interaction_date DESC NULLS LAST, created_at DESC
                 OFFSET $2
                 LIMIT $3",
                filter_string)
        )
        .bind::<SqlUuid, _>(journal_id)
        .bind::<diesel::sql_types::BigInt, _>(offset)
        .bind::<diesel::sql_types::BigInt, _>(limit)
        .load::<TraceRow>(&mut conn)?;

        Ok((rows.into_iter().map(Trace::from).collect(), total))
    }

    pub fn get_shared_for_journal_paginated(
        viewer_user_id: Uuid,
        journal_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<JournalTraceView>, i64), PpdcError> {
        let visible_post_ids = PostGrant::find_shared_post_ids_for_user(viewer_user_id, pool)?;
        if visible_post_ids.is_empty() {
            return Ok((vec![], 0));
        }

        let mut conn = pool.get()?;

        let total = traces::table
            .inner_join(posts::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(traces::journal_id.eq(journal_id))
            .filter(traces::status.eq(TraceStatus::Finalized.to_db()))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .filter(posts::id.eq_any(visible_post_ids.clone()))
            .count()
            .get_result::<i64>(&mut conn)?;

        let rows = traces::table
            .inner_join(posts::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(traces::journal_id.eq(journal_id))
            .filter(traces::status.eq(TraceStatus::Finalized.to_db()))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .filter(posts::id.eq_any(visible_post_ids))
            .select((
                traces::id,
                traces::journal_id,
                traces::title,
                traces::content,
                traces::image_asset_id,
                traces::interaction_date,
                traces::created_at,
                traces::updated_at,
            ))
            .order(traces::interaction_date.desc())
            .then_order_by(traces::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<(
                Uuid,
                Uuid,
                String,
                String,
                Option<Uuid>,
                NaiveDateTime,
                NaiveDateTime,
                NaiveDateTime,
            )>(&mut conn)?;

        let traces = rows
            .into_iter()
            .map(
                |(
                    id,
                    journal_id,
                    title,
                    content,
                    image_asset_id,
                    interaction_date,
                    created_at,
                    updated_at,
                )| JournalTraceView {
                    id,
                    journal_id,
                    title,
                    content,
                    image_asset_id,
                    seen: false,
                    last_seen_at: None,
                    interaction_date,
                    created_at,
                    updated_at,
                },
            )
            .collect();

        Ok((traces, total))
    }

    pub fn get_non_empty_drafts_for_user_paginated(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Trace>, i64), PpdcError> {
        let mut conn = pool.get()?;

        let total = diesel::sql_query(
            "SELECT COUNT(*)::bigint AS count
             FROM traces
             WHERE user_id = $1
               AND trace_type = 'USER_TRACE'
               AND status = 'DRAFT'
               AND NULLIF(BTRIM(content), '') IS NOT NULL",
        )
        .bind::<SqlUuid, _>(user_id)
        .get_result::<CountRow>(&mut conn)?
        .count;

        let rows = diesel::sql_query(
            "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE user_id = $1
               AND trace_type = 'USER_TRACE'
               AND status = 'DRAFT'
               AND NULLIF(BTRIM(content), '') IS NOT NULL
             ORDER BY updated_at DESC, created_at DESC
             OFFSET $2
             LIMIT $3",
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<diesel::sql_types::BigInt, _>(offset)
        .bind::<diesel::sql_types::BigInt, _>(limit)
        .load::<TraceRow>(&mut conn)?;

        Ok((rows.into_iter().map(Trace::from).collect(), total))
    }

    pub fn get_expired_drafts_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Trace>, PpdcError> {
        let mut conn = pool.get()?;

        let rows = diesel::sql_query(
            "SELECT id, derived_from_trace_id, title, subtitle, interaction_date, content, is_encrypted, encryption_metadata::text AS encryption_metadata, image_asset_id, sharing_sensitivity, timeout_start_at, timeout_at, journal_id, user_id, trace_type, status, start_writing_at, finalized_at, created_at, updated_at
             FROM traces
             WHERE user_id = $1
               AND trace_type = 'USER_TRACE'
               AND status = 'DRAFT'
               AND timeout_at IS NOT NULL
               AND timeout_at <= NOW()
             ORDER BY timeout_at ASC, created_at ASC",
        )
        .bind::<SqlUuid, _>(user_id)
        .load::<TraceRow>(&mut conn)?;

        Ok(rows.into_iter().map(Trace::from).collect())
    }
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
}

#[derive(Debug, Clone)]
pub struct NewTrace {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub derived_from_trace_id: Option<Uuid>,
    pub is_encrypted: bool,
    pub encryption_metadata: Option<Value>,
    pub image_asset_id: Option<Uuid>,
    pub sharing_sensitivity: TraceSharingSensitivity,
    pub timeout_start_at: Option<DateTime<Utc>>,
    pub timeout_at: Option<DateTime<Utc>>,
    pub interaction_date: NaiveDateTime,
    pub user_id: Uuid,
    pub trace_type: TraceType,
    pub journal_id: Uuid,
    pub start_writing_at: NaiveDateTime,
}

impl NewTrace {
    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        interaction_date: NaiveDateTime,
        user_id: Uuid,
        journal_id: Uuid,
    ) -> NewTrace {
        NewTrace {
            title,
            subtitle,
            content,
            derived_from_trace_id: None,
            is_encrypted: false,
            encryption_metadata: None,
            image_asset_id: None,
            sharing_sensitivity: TraceSharingSensitivity::Normal,
            timeout_start_at: None,
            timeout_at: None,
            interaction_date,
            user_id,
            trace_type: TraceType::UserTrace,
            journal_id,
            start_writing_at: chrono::Utc::now().naive_utc(),
        }
    }
}
