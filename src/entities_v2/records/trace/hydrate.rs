use diesel::prelude::*;
use chrono::NaiveDateTime;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::schema::traces;

use super::model::{Trace, TraceStatus, TraceType};

type TraceTuple = (
    Uuid,
    String,
    String,
    Option<NaiveDateTime>,
    String,
    Option<Uuid>,
    Uuid,
    String,
    String,
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
        journal_id,
        user_id,
        trace_type_raw,
        status_raw,
        created_at,
        updated_at,
    ) = row;

    Trace {
        id,
        title,
        subtitle,
        interaction_date,
        content,
        journal_id,
        user_id,
        trace_type: TraceType::from_db(&trace_type_raw),
        status: TraceStatus::from_db(&status_raw),
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
                traces::journal_id.nullable(),
                traces::user_id,
                traces::trace_type,
                traces::status,
                traces::created_at,
                traces::updated_at,
            ))
            .first::<TraceTuple>(&mut conn)?;

        Ok(tuple_to_trace(row))
    }
}
