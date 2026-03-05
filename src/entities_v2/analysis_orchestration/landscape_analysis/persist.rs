use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::{landscape_analyses, landscape_landmarks, lens_analysis_scopes};
use crate::entities_v2::reference::Reference;

use super::model::{LandscapeAnalysis, LandscapeProcessingState, NewLandscapeAnalysis};

impl LandscapeAnalysis {
    pub fn update(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::update(landscape_analyses::table.filter(landscape_analyses::id.eq(self.id)))
            .set((
                landscape_analyses::title.eq(self.title),
                landscape_analyses::subtitle.eq(self.subtitle),
                landscape_analyses::plain_text_state_summary.eq(self.plain_text_state_summary),
                landscape_analyses::interaction_date.eq(self.interaction_date),
                landscape_analyses::period_start.eq(self.period_start),
                landscape_analyses::period_end.eq(self.period_end),
                landscape_analyses::landscape_analysis_type.eq(self.landscape_analysis_type.to_db()),
                landscape_analyses::processing_state.eq(self.processing_state.to_db()),
                landscape_analyses::parent_id.eq(self.parent_analysis_id),
                landscape_analyses::replayed_from_id.eq(self.replayed_from_id),
                landscape_analyses::analyzed_trace_id.eq(self.analyzed_trace_id),
                landscape_analyses::trace_mirror_id.eq(self.trace_mirror_id),
            ))
            .execute(&mut conn)?;
        LandscapeAnalysis::find_full_analysis(self.id, pool)
    }

    pub fn set_processing_state(
        mut self,
        state: LandscapeProcessingState,
        pool: &DbPool,
    ) -> Result<LandscapeAnalysis, PpdcError> {
        self.processing_state = state;
        self.update(pool)
    }

    pub fn delete(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::delete(landscape_analyses::table.filter(landscape_analyses::id.eq(self.id)))
            .execute(&mut conn)?;
        Ok(self)
    }
}

impl NewLandscapeAnalysis {
    pub fn create(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let id: Uuid = diesel::insert_into(landscape_analyses::table)
            .values((
                landscape_analyses::title.eq(self.title),
                landscape_analyses::subtitle.eq(self.subtitle),
                landscape_analyses::plain_text_state_summary.eq(self.plain_text_state_summary),
                landscape_analyses::interaction_date.eq(Some(self.interaction_date)),
                landscape_analyses::period_start.eq(self.period_start),
                landscape_analyses::period_end.eq(self.period_end),
                landscape_analyses::user_id.eq(self.user_id),
                landscape_analyses::landscape_analysis_type.eq(self.landscape_analysis_type.to_db()),
                landscape_analyses::processing_state.eq(LandscapeProcessingState::Pending.to_db()),
                landscape_analyses::parent_id.eq(self.parent_analysis_id),
                landscape_analyses::analyzed_trace_id.eq(self.analyzed_trace_id),
                landscape_analyses::replayed_from_id.eq(self.replayed_from_id),
                landscape_analyses::trace_mirror_id.eq(self.trace_mirror_id),
            ))
            .returning(landscape_analyses::id)
            .get_result(&mut conn)?;

        LandscapeAnalysis::find_full_analysis(id, pool)
    }

    pub fn create_for_lens(self, lens_id: Uuid, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let analysis_id: Uuid = conn.transaction::<Uuid, diesel::result::Error, _>(|conn| {
            let id: Uuid = diesel::insert_into(landscape_analyses::table)
                .values((
                    landscape_analyses::title.eq(self.title.clone()),
                    landscape_analyses::subtitle.eq(self.subtitle.clone()),
                    landscape_analyses::plain_text_state_summary.eq(self.plain_text_state_summary.clone()),
                    landscape_analyses::interaction_date.eq(Some(self.interaction_date)),
                    landscape_analyses::period_start.eq(self.period_start),
                    landscape_analyses::period_end.eq(self.period_end),
                    landscape_analyses::user_id.eq(self.user_id),
                    landscape_analyses::landscape_analysis_type.eq(self.landscape_analysis_type.to_db()),
                    landscape_analyses::processing_state.eq(LandscapeProcessingState::Pending.to_db()),
                    landscape_analyses::parent_id.eq(self.parent_analysis_id),
                    landscape_analyses::analyzed_trace_id.eq(self.analyzed_trace_id),
                    landscape_analyses::replayed_from_id.eq(self.replayed_from_id),
                    landscape_analyses::trace_mirror_id.eq(self.trace_mirror_id),
                ))
                .returning(landscape_analyses::id)
                .get_result(conn)?;

            diesel::insert_into(lens_analysis_scopes::table)
                .values((
                    lens_analysis_scopes::id.eq(Uuid::new_v4()),
                    lens_analysis_scopes::lens_id.eq(lens_id),
                    lens_analysis_scopes::landscape_analysis_id.eq(id),
                ))
                .on_conflict((
                    lens_analysis_scopes::lens_id,
                    lens_analysis_scopes::landscape_analysis_id,
                ))
                .do_nothing()
                .execute(conn)?;
            Ok(id)
        })?;

        LandscapeAnalysis::find_full_analysis(analysis_id, pool)
    }
}

pub fn delete_leaf_and_cleanup(
    id: Uuid,
    pool: &DbPool,
) -> Result<Option<LandscapeAnalysis>, PpdcError> {
    let landscape_analysis = LandscapeAnalysis::find_full_analysis(id, pool)?;
    let children_landscape_analyses = landscape_analysis.get_children_landscape_analyses(pool)?;
    let heading_lens = landscape_analysis.get_heading_lens(pool)?;
    if !children_landscape_analyses.is_empty() || !heading_lens.is_empty() {
        return Ok(None);
    }

    Reference::delete_for_landscape_analysis(id, pool)?;
    let deleted = landscape_analysis.delete(pool)?;
    Ok(Some(deleted))
}

pub fn find_last_analysis_resource(
    user_id: Uuid,
    pool: &DbPool,
) -> Result<Option<LandscapeAnalysis>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    let maybe_id = landscape_analyses::table
        .filter(landscape_analyses::user_id.eq(user_id))
        .order(landscape_analyses::interaction_date.desc().nulls_last())
        .then_order_by(landscape_analyses::created_at.desc())
        .select(landscape_analyses::id)
        .first::<Uuid>(&mut conn)
        .optional()?;
    match maybe_id {
        Some(id) => Ok(Some(LandscapeAnalysis::find_full_analysis(id, pool)?)),
        None => Ok(None),
    }
}

pub fn add_landmark_ref(
    landscape_analysis_id: Uuid,
    landmark_id: Uuid,
    _user_id: Uuid,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    diesel::insert_into(landscape_landmarks::table)
        .values((
            landscape_landmarks::landscape_analysis_id.eq(landscape_analysis_id),
            landscape_landmarks::landmark_id.eq(landmark_id),
            landscape_landmarks::relation_type.eq("REFERENCED"),
        ))
        .on_conflict((
            landscape_landmarks::landscape_analysis_id,
            landscape_landmarks::landmark_id,
            landscape_landmarks::relation_type,
        ))
        .do_nothing()
        .execute(&mut conn)?;
    Ok(())
}
