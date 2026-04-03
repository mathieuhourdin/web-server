use chrono::NaiveDateTime;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::{Nullable, Text};
use serde_json::Value;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::traces;

use super::model::{Trace, TraceStatus, TraceType};

type TraceTuple = (
    Uuid,
    String,
    String,
    NaiveDateTime,
    String,
    bool,
    Option<String>,
    Option<Uuid>,
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
        title,
        subtitle,
        interaction_date,
        content,
        is_encrypted,
        encryption_metadata_json,
        image_asset_id,
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
        title,
        subtitle,
        interaction_date,
        content,
        is_encrypted,
        encryption_metadata: encryption_metadata_json
            .and_then(|json| serde_json::from_str::<Value>(&json).ok()),
        image_asset_id,
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
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let row = traces::table
            .filter(traces::id.eq(id))
            .select((
                traces::id,
                traces::title,
                traces::subtitle,
                traces::interaction_date,
                traces::content,
                traces::is_encrypted,
                sql::<Nullable<Text>>("encryption_metadata::text"),
                traces::image_asset_id,
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
