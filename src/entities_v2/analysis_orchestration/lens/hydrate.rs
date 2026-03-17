use std::collections::HashSet;

use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::schema::{lens_analysis_scopes, lenses};

use super::model::{Lens, LensProcessingState};

type LensTuple = (
    Uuid,
    Uuid,
    String,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    bool,
);

fn tuple_to_lens(row: LensTuple) -> Lens {
    let (
        id,
        user_id,
        processing_state_raw,
        fork_landscape_id,
        current_landscape_id,
        target_trace_id,
        autoplay,
    ) = row;

    Lens {
        id,
        user_id: Some(user_id),
        processing_state: LensProcessingState::from_db(&processing_state_raw),
        fork_landscape_id,
        current_landscape_id,
        target_trace_id,
        autoplay,
    }
}

fn select_lens_columns() -> (
    lenses::id,
    lenses::user_id,
    lenses::processing_state,
    lenses::fork_landscape_id,
    lenses::current_landscape_id,
    lenses::target_trace_id,
    lenses::autoplay,
) {
    (
        lenses::id,
        lenses::user_id,
        lenses::processing_state,
        lenses::fork_landscape_id,
        lenses::current_landscape_id,
        lenses::target_trace_id,
        lenses::autoplay,
    )
}

impl Lens {
    pub fn find_full_lens(id: Uuid, pool: &DbPool) -> Result<Lens, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let row = lenses::table
            .filter(lenses::id.eq(id))
            .select(select_lens_columns())
            .first::<LensTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_lens)
            .ok_or_else(|| PpdcError::new(404, ErrorType::ApiError, "Lens not found".to_string()))
    }

    pub fn get_user_lenses(user_id: Uuid, pool: &DbPool) -> Result<Vec<Lens>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = lenses::table
            .filter(lenses::user_id.eq(user_id))
            .select(select_lens_columns())
            .order(lenses::updated_at.desc())
            .load::<LensTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_lens).collect())
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
