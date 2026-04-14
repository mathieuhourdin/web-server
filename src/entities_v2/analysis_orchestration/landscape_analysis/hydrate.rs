use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::{element::Element, landmark::Landmark, lens::Lens};
use crate::schema::{
    landscape_analyses, landscape_analysis_inputs, lens_analysis_scopes, lens_heads,
};

use super::model::{
    LandscapeAnalysis, LandscapeAnalysisInput, LandscapeAnalysisInputType, LandscapeAnalysisType,
    LandscapeProcessingState,
};

type LandscapeAnalysisTuple = (
    Uuid,
    String,
    String,
    String,
    Option<NaiveDateTime>,
    NaiveDateTime,
    NaiveDateTime,
    Uuid,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

type LandscapeAnalysisInputTuple = (
    Uuid,
    Uuid,
    Option<Uuid>,
    Option<Uuid>,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

/// Maps a raw SQL row into the API/domain shape used for landscape analyses.
fn tuple_to_analysis(row: LandscapeAnalysisTuple) -> LandscapeAnalysis {
    let (
        id,
        title,
        subtitle,
        plain_text_state_summary,
        interaction_date,
        period_start,
        period_end,
        user_id,
        parent_id,
        replayed_from_id,
        analyzed_trace_id,
        trace_mirror_id,
        landscape_analysis_type_raw,
        processing_state_raw,
        created_at,
        updated_at,
    ) = row;

    LandscapeAnalysis {
        id,
        title,
        subtitle,
        plain_text_state_summary,
        interaction_date,
        period_start,
        period_end,
        user_id,
        parent_analysis_id: parent_id,
        replayed_from_id,
        analyzed_trace_id,
        trace_mirror_id,
        landscape_analysis_type: LandscapeAnalysisType::from_db(&landscape_analysis_type_raw),
        processing_state: LandscapeProcessingState::from_db(&processing_state_raw),
        created_at,
        updated_at,
    }
}

/// Maps a raw SQL row into the domain shape used for analysis inputs.
fn tuple_to_analysis_input(row: LandscapeAnalysisInputTuple) -> LandscapeAnalysisInput {
    let (
        id,
        landscape_analysis_id,
        trace_id,
        trace_mirror_id,
        input_type_raw,
        created_at,
        updated_at,
    ) = row;

    LandscapeAnalysisInput {
        id,
        landscape_analysis_id,
        trace_id,
        trace_mirror_id,
        input_type: LandscapeAnalysisInputType::from_db(&input_type_raw),
        created_at,
        updated_at,
    }
}

/// Centralizes the analysis projection used by hydrate queries so tuple decoding stays consistent.
fn select_analysis_columns() -> (
    landscape_analyses::id,
    landscape_analyses::title,
    landscape_analyses::subtitle,
    landscape_analyses::plain_text_state_summary,
    landscape_analyses::interaction_date,
    landscape_analyses::period_start,
    landscape_analyses::period_end,
    landscape_analyses::user_id,
    landscape_analyses::parent_id,
    landscape_analyses::replayed_from_id,
    landscape_analyses::analyzed_trace_id,
    landscape_analyses::trace_mirror_id,
    landscape_analyses::landscape_analysis_type,
    landscape_analyses::processing_state,
    landscape_analyses::created_at,
    landscape_analyses::updated_at,
) {
    (
        landscape_analyses::id,
        landscape_analyses::title,
        landscape_analyses::subtitle,
        landscape_analyses::plain_text_state_summary,
        landscape_analyses::interaction_date,
        landscape_analyses::period_start,
        landscape_analyses::period_end,
        landscape_analyses::user_id,
        landscape_analyses::parent_id,
        landscape_analyses::replayed_from_id,
        landscape_analyses::analyzed_trace_id,
        landscape_analyses::trace_mirror_id,
        landscape_analyses::landscape_analysis_type,
        landscape_analyses::processing_state,
        landscape_analyses::created_at,
        landscape_analyses::updated_at,
    )
}

impl LandscapeAnalysis {
    /// Resolves the direct parent analysis, which represents the previous state in the lens timeline.
    pub fn find_full_parent(&self, pool: &DbPool) -> Result<Option<LandscapeAnalysis>, PpdcError> {
        let parent_analysis_id = self.parent_analysis_id;
        if let Some(parent_analysis_id) = parent_analysis_id {
            let parent_analysis = LandscapeAnalysis::find_full_analysis(parent_analysis_id, pool)?;
            Ok(Some(parent_analysis))
        } else {
            Ok(None)
        }
    }

    /// Walks the parent chain to rebuild the full ancestry of an analysis in chronological reverse order.
    pub fn find_all_parents(&self, pool: &DbPool) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
        let mut parents = vec![];
        let mut current_analysis = self.clone();
        while let Some(parent_analysis) = current_analysis.find_full_parent(pool)? {
            parents.push(parent_analysis.clone());
            current_analysis = parent_analysis;
        }
        Ok(parents)
    }

    /// Loads one analysis with all persisted fields, which is the base read path for analysis routes and workers.
    pub fn find_full_analysis(id: Uuid, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let row = landscape_analyses::table
            .filter(landscape_analyses::id.eq(id))
            .select(select_analysis_columns())
            .first::<LandscapeAnalysisTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_analysis).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Landscape analysis not found".to_string(),
            )
        })
    }

    /// Returns every analysis directly tied to a given source trace, ordered from most recent run to oldest.
    pub fn find_by_trace(
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = landscape_analyses::table
            .filter(landscape_analyses::analyzed_trace_id.eq(trace_id))
            .select(select_analysis_columns())
            .order(landscape_analyses::interaction_date.desc().nulls_last())
            .then_order_by(landscape_analyses::created_at.desc())
            .load::<LandscapeAnalysisTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_analysis).collect())
    }

    /// Returns the landmarks visible from this analysis, optionally restricted to one relation role.
    pub fn get_landmarks(
        &self,
        relation_type: Option<&str>,
        pool: &DbPool,
    ) -> Result<Vec<Landmark>, PpdcError> {
        Landmark::get_for_landscape_analysis(self.id, relation_type, pool)
    }

    /// Loads the elements produced by this analysis run, which are the trace-scoped analytic outputs.
    pub fn get_elements(&self, pool: &DbPool) -> Result<Vec<Element>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let element_ids = crate::schema::elements::table
            .filter(crate::schema::elements::analysis_id.eq(self.id))
            .select(crate::schema::elements::id)
            .load::<Uuid>(&mut conn)?;
        element_ids
            .into_iter()
            .map(|id| Element::find_full(id, pool))
            .collect()
    }

    /// Returns direct child analyses that continue this analysis state in later runs.
    pub fn get_children_landscape_analyses(
        &self,
        pool: &DbPool,
    ) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = landscape_analyses::table
            .filter(landscape_analyses::parent_id.eq(self.id))
            .select(landscape_analyses::id)
            .load::<Uuid>(&mut conn)?;
        rows.into_iter()
            .map(|id| LandscapeAnalysis::find_full_analysis(id, pool))
            .collect()
    }

    /// Returns the lenses whose current head points to this analysis.
    pub fn get_heading_lens(&self, pool: &DbPool) -> Result<Vec<Lens>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let lens_ids = lens_heads::table
            .filter(lens_heads::landscape_analysis_id.eq(self.id))
            .select(lens_heads::lens_id)
            .load::<Uuid>(&mut conn)?;
        lens_ids
            .into_iter()
            .map(|id| Lens::find_full_lens(id, pool))
            .collect()
    }

    /// Returns every lens scope that includes this analysis, including forked descendants.
    pub fn get_scoped_lenses(&self, pool: &DbPool) -> Result<Vec<Lens>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let lens_ids = lens_analysis_scopes::table
            .filter(lens_analysis_scopes::landscape_analysis_id.eq(self.id))
            .select(lens_analysis_scopes::lens_id)
            .load::<Uuid>(&mut conn)?;
        lens_ids
            .into_iter()
            .map(|id| Lens::find_full_lens(id, pool))
            .collect()
    }

    /// Returns the traces and trace mirrors explicitly recorded as inputs to this analysis run.
    pub fn get_inputs(&self, pool: &DbPool) -> Result<Vec<LandscapeAnalysisInput>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let rows = landscape_analysis_inputs::table
            .filter(landscape_analysis_inputs::landscape_analysis_id.eq(self.id))
            .select((
                landscape_analysis_inputs::id,
                landscape_analysis_inputs::landscape_analysis_id,
                landscape_analysis_inputs::trace_id,
                landscape_analysis_inputs::trace_mirror_id,
                landscape_analysis_inputs::input_type,
                landscape_analysis_inputs::created_at,
                landscape_analysis_inputs::updated_at,
            ))
            .order(landscape_analysis_inputs::created_at.asc())
            .load::<LandscapeAnalysisInputTuple>(&mut conn)?;

        Ok(rows.into_iter().map(tuple_to_analysis_input).collect())
    }
}
