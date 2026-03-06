use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::analysis_summaries;

use super::model::{AnalysisSummary, AnalysisSummaryType};

type AnalysisSummaryTuple = (
    Uuid,
    Uuid,
    Uuid,
    String,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

fn tuple_to_analysis_summary(row: AnalysisSummaryTuple) -> AnalysisSummary {
    let (
        id,
        landscape_analysis_id,
        user_id,
        summary_type_raw,
        title,
        content,
        created_at,
        updated_at,
    ) = row;

    AnalysisSummary {
        id,
        landscape_analysis_id,
        user_id,
        summary_type: AnalysisSummaryType::from_db(&summary_type_raw),
        title,
        content,
        created_at,
        updated_at,
    }
}

fn select_analysis_summary_columns() -> (
    analysis_summaries::id,
    analysis_summaries::landscape_analysis_id,
    analysis_summaries::user_id,
    analysis_summaries::summary_type,
    analysis_summaries::title,
    analysis_summaries::content,
    analysis_summaries::created_at,
    analysis_summaries::updated_at,
) {
    (
        analysis_summaries::id,
        analysis_summaries::landscape_analysis_id,
        analysis_summaries::user_id,
        analysis_summaries::summary_type,
        analysis_summaries::title,
        analysis_summaries::content,
        analysis_summaries::created_at,
        analysis_summaries::updated_at,
    )
}

impl AnalysisSummary {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<AnalysisSummary, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let row = analysis_summaries::table
            .filter(analysis_summaries::id.eq(id))
            .select(select_analysis_summary_columns())
            .first::<AnalysisSummaryTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_analysis_summary).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Analysis summary not found".to_string(),
            )
        })
    }

    pub fn find_for_analysis(
        landscape_analysis_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<AnalysisSummary>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = analysis_summaries::table
            .filter(analysis_summaries::landscape_analysis_id.eq(landscape_analysis_id))
            .select(select_analysis_summary_columns())
            .order(analysis_summaries::created_at.desc())
            .load::<AnalysisSummaryTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_analysis_summary).collect())
    }
}
