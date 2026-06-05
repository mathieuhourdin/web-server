use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::{Nullable, Text, Timestamptz};
use serde_json::Value;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::traces;

use super::model::{Trace, TraceSharingSensitivity, TraceStatus, TraceType};

type TraceTuple = (
    Uuid,
    Option<Uuid>,
    String,
    String,
    NaiveDateTime,
    String,
    bool,
    Option<String>,
    Option<Uuid>,
    String,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    Option<Uuid>,
    Uuid,
    String,
    String,
    NaiveDateTime,
    Option<NaiveDateTime>,
    NaiveDateTime,
    NaiveDateTime,
);

fn tuple_to_trace(row: TraceTuple) -> Trace {
    let (
        id,
        derived_from_trace_id,
        title,
        subtitle,
        interaction_date,
        content,
        is_encrypted,
        encryption_metadata_json,
        content_image_asset_id,
        sharing_sensitivity_raw,
        timeout_start_at,
        timeout_at,
        journal_id,
        user_id,
        trace_type_raw,
        status_raw,
        start_writing_at,
        finalized_at,
        created_at,
        updated_at,
    ) = row;

    Trace {
        id,
        derived_from_trace_id,
        title,
        subtitle,
        interaction_date,
        content,
        is_encrypted,
        encryption_metadata: encryption_metadata_json
            .and_then(|json| serde_json::from_str::<Value>(&json).ok()),
        content_image_asset_id,
        sharing_sensitivity: TraceSharingSensitivity::from_db(&sharing_sensitivity_raw),
        timeout_start_at,
        timeout_at,
        journal_id,
        user_id,
        trace_type: TraceType::from_db(&trace_type_raw),
        status: TraceStatus::from_db(&status_raw),
        start_writing_at,
        finalized_at,
        created_at,
        updated_at,
    }
}

impl Trace {
    pub fn find_full_trace(id: Uuid, pool: &DbPool) -> Result<Trace, PpdcError> {
        let mut conn = pool.get()?;

        let row = traces::table
            .filter(traces::id.eq(id))
            .select((
                traces::id,
                traces::derived_from_trace_id,
                traces::title,
                traces::subtitle,
                traces::interaction_date,
                traces::content,
                traces::is_encrypted,
                sql::<Nullable<Text>>("encryption_metadata::text"),
                traces::content_image_asset_id,
                traces::sharing_sensitivity,
                sql::<Nullable<Timestamptz>>("timeout_start_at"),
                sql::<Nullable<Timestamptz>>("timeout_at"),
                traces::journal_id.nullable(),
                traces::user_id,
                traces::trace_type,
                traces::status,
                traces::start_writing_at,
                traces::finalized_at,
                traces::created_at,
                traces::updated_at,
            ))
            .first::<TraceTuple>(&mut conn)?;

        Ok(tuple_to_trace(row))
    }
}
