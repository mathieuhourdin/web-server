use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text, Timestamp, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::schema::{landscape_analyses, lens_analysis_scopes, lens_heads};
use crate::entities_v2::{element::Element, landmark::Landmark, lens::Lens};

use super::model::{LandscapeAnalysis, LandscapeProcessingState};

#[derive(QueryableByName)]
struct LandscapeAnalysisRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    subtitle: String,
    #[diesel(sql_type = Text)]
    plain_text_state_summary: String,
    #[diesel(sql_type = Nullable<Timestamp>)]
    interaction_date: Option<NaiveDateTime>,
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    parent_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    replayed_from_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    analyzed_trace_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    trace_mirror_id: Option<Uuid>,
    #[diesel(sql_type = Text)]
    processing_state: String,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
}

fn row_to_analysis(row: LandscapeAnalysisRow) -> LandscapeAnalysis {
    LandscapeAnalysis {
        id: row.id,
        title: row.title,
        subtitle: row.subtitle,
        plain_text_state_summary: row.plain_text_state_summary,
        interaction_date: row.interaction_date,
        user_id: row.user_id,
        parent_analysis_id: row.parent_id,
        replayed_from_id: row.replayed_from_id,
        analyzed_trace_id: row.analyzed_trace_id,
        trace_mirror_id: row.trace_mirror_id,
        processing_state: LandscapeProcessingState::from_db(&row.processing_state),
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

impl LandscapeAnalysis {
    pub fn find_full_parent(&self, pool: &DbPool) -> Result<Option<LandscapeAnalysis>, PpdcError> {
        let parent_analysis_id = self.parent_analysis_id;
        if let Some(parent_analysis_id) = parent_analysis_id {
            let parent_analysis = LandscapeAnalysis::find_full_analysis(parent_analysis_id, pool)?;
            Ok(Some(parent_analysis))
        } else {
            Ok(None)
        }
    }

    pub fn find_all_parents(&self, pool: &DbPool) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
        let mut parents = vec![];
        let mut current_analysis = self.clone();
        while let Some(parent_analysis) = current_analysis.find_full_parent(pool)? {
            parents.push(parent_analysis.clone());
            current_analysis = parent_analysis;
        }
        Ok(parents)
    }

    pub fn find_full_analysis(id: Uuid, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let mut rows = sql_query(
            r#"
            SELECT id, title, subtitle, plain_text_state_summary, interaction_date, user_id,
                   parent_id, replayed_from_id, analyzed_trace_id, trace_mirror_id,
                   processing_state, created_at, updated_at
            FROM landscape_analyses
            WHERE id = $1
            "#,
        )
        .bind::<SqlUuid, _>(id)
        .load::<LandscapeAnalysisRow>(&mut conn)?;
        rows.pop().map(row_to_analysis).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Landscape analysis not found".to_string(),
            )
        })
    }

    pub fn find_by_trace(trace_id: Uuid, pool: &DbPool) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = sql_query(
            r#"
            SELECT id, title, subtitle, plain_text_state_summary, interaction_date, user_id,
                   parent_id, replayed_from_id, analyzed_trace_id, trace_mirror_id,
                   processing_state, created_at, updated_at
            FROM landscape_analyses
            WHERE analyzed_trace_id = $1
            ORDER BY interaction_date DESC NULLS LAST, created_at DESC
            "#,
        )
        .bind::<SqlUuid, _>(trace_id)
        .load::<LandscapeAnalysisRow>(&mut conn)?;
        Ok(rows.into_iter().map(row_to_analysis).collect())
    }

    pub fn get_landmarks(
        &self,
        relation_type: Option<&str>,
        pool: &DbPool,
    ) -> Result<Vec<Landmark>, PpdcError> {
        Landmark::get_for_landscape_analysis(self.id, relation_type, pool)
    }

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
}
