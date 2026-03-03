use diesel::prelude::*;
use diesel::sql_types::Uuid as SqlUuid;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;

use super::model::{Trace, TraceRow};

impl Trace {
    pub fn find_full_trace(id: Uuid, pool: &DbPool) -> Result<Trace, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let row = diesel::sql_query(
            "SELECT id, title, subtitle, interaction_date, content, journal_id, user_id, trace_type, status, created_at, updated_at
             FROM traces
             WHERE id = $1",
        )
        .bind::<SqlUuid, _>(id)
        .get_result::<TraceRow>(&mut conn)?;

        Ok(row.into())
    }
}
