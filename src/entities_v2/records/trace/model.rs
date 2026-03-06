use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types::{Nullable, Text, Timestamp, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;

#[derive(Deserialize)]
pub struct NewTraceDto {
    pub content: String,
    pub journal_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Trace {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub content: String,
    pub journal_id: Option<Uuid>,
    pub user_id: Uuid,
    pub trace_type: TraceType,
    pub status: TraceStatus,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(QueryableByName, Debug)]
pub(crate) struct TraceRow {
    #[diesel(sql_type = SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type = Text)]
    pub title: String,
    #[diesel(sql_type = Text)]
    pub subtitle: String,
    #[diesel(sql_type = Nullable<Timestamp>)]
    pub interaction_date: Option<NaiveDateTime>,
    #[diesel(sql_type = Text)]
    pub content: String,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    pub journal_id: Option<Uuid>,
    #[diesel(sql_type = SqlUuid)]
    pub user_id: Uuid,
    #[diesel(sql_type = Text)]
    pub trace_type: String,
    #[diesel(sql_type = Text)]
    pub status: String,
    #[diesel(sql_type = Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    pub updated_at: NaiveDateTime,
}

impl From<TraceRow> for Trace {
    fn from(row: TraceRow) -> Self {
        Trace {
            id: row.id,
            title: row.title,
            subtitle: row.subtitle,
            interaction_date: row.interaction_date,
            content: row.content,
            journal_id: row.journal_id,
            user_id: row.user_id,
            trace_type: TraceType::from_db(&row.trace_type),
            status: TraceStatus::from_db(&row.status),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceType {
    BioTrace,
    WorkspaceTrace,
    UserTrace,
    HighLevelProjectsDefinition,
}

impl TraceType {
    pub fn to_db(self) -> &'static str {
        match self {
            TraceType::UserTrace => "USER_TRACE",
            TraceType::BioTrace => "BIO_TRACE",
            TraceType::WorkspaceTrace => "WORKSPACE_TRACE",
            TraceType::HighLevelProjectsDefinition => "HIGH_LEVEL_PROJECTS_DEFINITION",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "BIO_TRACE" | "btrc" | "BTRC" => TraceType::BioTrace,
            "WORKSPACE_TRACE" | "wtrc" | "WTRC" => TraceType::WorkspaceTrace,
            "HIGH_LEVEL_PROJECTS_DEFINITION" | "hlpd" | "HLPD" => {
                TraceType::HighLevelProjectsDefinition
            }
            "USER_TRACE" | "trce" | "TRCE" => TraceType::UserTrace,
            _ => TraceType::UserTrace,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceStatus {
    Draft,
    Finalized,
    Archived,
}

impl TraceStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            TraceStatus::Draft => "DRAFT",
            TraceStatus::Finalized => "FINALIZED",
            TraceStatus::Archived => "ARCHIVED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "FINALIZED" => TraceStatus::Finalized,
            "ARCHIVED" => TraceStatus::Archived,
            _ => TraceStatus::Draft,
        }
    }
}

impl Trace {
    fn find_first_user_trace_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Trace>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let row = diesel::sql_query(
            "SELECT id, title, subtitle, interaction_date, content, journal_id, user_id, trace_type, status, created_at, updated_at
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
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let row = diesel::sql_query(
            "SELECT id, title, subtitle, interaction_date, content, journal_id, user_id, trace_type, status, created_at, updated_at
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

        let from = start_trace
            .interaction_date
            .unwrap_or(start_trace.created_at);
        let to = end_trace.interaction_date.unwrap_or(end_trace.created_at);

        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let rows = diesel::sql_query(
            "SELECT id, title, subtitle, interaction_date, content, journal_id, user_id, trace_type, status, created_at, updated_at
             FROM traces
             WHERE user_id = $1
               AND COALESCE(interaction_date, created_at) BETWEEN $2 AND $3
             ORDER BY COALESCE(interaction_date, created_at) ASC, created_at ASC",
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
        let until = trace.interaction_date.unwrap_or(trace.created_at);

        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let rows = diesel::sql_query(
            "SELECT id, title, subtitle, interaction_date, content, journal_id, user_id, trace_type, status, created_at, updated_at
             FROM traces
             WHERE user_id = $1
               AND COALESCE(interaction_date, created_at) <= $2
             ORDER BY COALESCE(interaction_date, created_at) ASC, created_at ASC",
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Timestamp, _>(until)
        .load::<TraceRow>(&mut conn)?;

        Ok(rows.into_iter().map(Trace::from).collect())
    }

    pub fn get_first(user_id: Uuid, pool: &DbPool) -> Result<Option<Trace>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let latest_hlp = diesel::sql_query(
            "SELECT id, title, subtitle, interaction_date, content, journal_id, user_id, trace_type, status, created_at, updated_at
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
            "SELECT id, title, subtitle, interaction_date, content, journal_id, user_id, trace_type, status, created_at, updated_at
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
            let mut conn = pool
                .get()
                .expect("Failed to get a connection from the pool");

            let latest_bio = diesel::sql_query(
                "SELECT id, title, subtitle, interaction_date, content, journal_id, user_id, trace_type, status, created_at, updated_at
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

        let reference_ts = trace.interaction_date.unwrap_or(trace.created_at);
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let next = diesel::sql_query(
            "SELECT id, title, subtitle, interaction_date, content, journal_id, user_id, trace_type, status, created_at, updated_at
             FROM traces
             WHERE user_id = $1
               AND trace_type = 'USER_TRACE'
               AND COALESCE(interaction_date, created_at) > $2
             ORDER BY COALESCE(interaction_date, created_at) ASC, created_at ASC
             LIMIT 1",
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Timestamp, _>(reference_ts)
        .get_result::<TraceRow>(&mut conn)
        .optional()?;

        Ok(next.map(Trace::from))
    }

    pub fn get_all_for_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<Trace>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let rows = diesel::sql_query(
            "SELECT id, title, subtitle, interaction_date, content, journal_id, user_id, trace_type, status, created_at, updated_at
             FROM traces
             WHERE user_id = $1
             ORDER BY interaction_date DESC NULLS LAST, created_at DESC",
        )
        .bind::<SqlUuid, _>(user_id)
        .load::<TraceRow>(&mut conn)?;

        Ok(rows.into_iter().map(Trace::from).collect())
    }

    pub fn get_all_for_journal(journal_id: Uuid, pool: &DbPool) -> Result<Vec<Trace>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let rows = diesel::sql_query(
            "SELECT id, title, subtitle, interaction_date, content, journal_id, user_id, trace_type, status, created_at, updated_at
             FROM traces
             WHERE journal_id = $1
             ORDER BY interaction_date DESC NULLS LAST, created_at DESC",
        )
        .bind::<SqlUuid, _>(journal_id)
        .load::<TraceRow>(&mut conn)?;

        Ok(rows.into_iter().map(Trace::from).collect())
    }
}

#[derive(Debug, Clone)]
pub struct NewTrace {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub user_id: Uuid,
    pub trace_type: TraceType,
    pub journal_id: Uuid,
}

impl NewTrace {
    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        interaction_date: Option<NaiveDateTime>,
        user_id: Uuid,
        journal_id: Uuid,
    ) -> NewTrace {
        NewTrace {
            title,
            subtitle,
            content,
            interaction_date,
            user_id,
            trace_type: TraceType::UserTrace,
            journal_id,
        }
    }
}
