use std::collections::HashSet;

use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Bool, Nullable, Text, Timestamp, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::schema::lens_analysis_scopes;
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;

use super::model::{Lens, LensProcessingState};

#[derive(QueryableByName)]
struct LensRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type = Text)]
    processing_state: String,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    fork_landscape_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    current_landscape_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    target_trace_id: Option<Uuid>,
    #[diesel(sql_type = Bool)]
    autoplay: bool,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
}

fn row_to_lens(row: LensRow) -> Lens {
    let _ = row.created_at;
    let _ = row.updated_at;
    Lens {
        id: row.id,
        user_id: Some(row.user_id),
        processing_state: LensProcessingState::from_db(&row.processing_state),
        fork_landscape_id: row.fork_landscape_id,
        current_landscape_id: row.current_landscape_id,
        target_trace_id: row.target_trace_id,
        autoplay: row.autoplay,
    }
}

impl Lens {
    pub fn find_full_lens(id: Uuid, pool: &DbPool) -> Result<Lens, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let mut rows = sql_query(
            r#"
            SELECT id, user_id, processing_state, fork_landscape_id, current_landscape_id,
                   target_trace_id, autoplay, created_at, updated_at
            FROM lenses
            WHERE id = $1
            "#,
        )
        .bind::<SqlUuid, _>(id)
        .load::<LensRow>(&mut conn)?;
        rows.pop().map(row_to_lens).ok_or_else(|| {
            PpdcError::new(404, ErrorType::ApiError, "Lens not found".to_string())
        })
    }

    pub fn get_user_lenses(user_id: Uuid, pool: &DbPool) -> Result<Vec<Lens>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = sql_query(
            r#"
            SELECT id, user_id, processing_state, fork_landscape_id, current_landscape_id,
                   target_trace_id, autoplay, created_at, updated_at
            FROM lenses
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind::<SqlUuid, _>(user_id)
        .load::<LensRow>(&mut conn)?;
        Ok(rows.into_iter().map(row_to_lens).collect())
    }

    pub fn get_analysis_scope_ids(&self, pool: &DbPool) -> Result<Vec<Uuid>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let mut seen = HashSet::new();
        let ids = lens_analysis_scopes::table
            .filter(lens_analysis_scopes::lens_id.eq(self.id))
            .select(lens_analysis_scopes::landscape_analysis_id)
            .load::<Uuid>(&mut conn)?
            .into_iter()
            .filter(|id| seen.insert(*id))
            .collect::<Vec<_>>();
        Ok(ids)
    }

    pub fn get_analysis_scope(&self, pool: &DbPool) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
        let analysis_ids = self.get_analysis_scope_ids(pool)?;
        analysis_ids
            .into_iter()
            .map(|analysis_id| LandscapeAnalysis::find_full_analysis(analysis_id, pool))
            .collect::<Result<Vec<_>, _>>()
    }
}
