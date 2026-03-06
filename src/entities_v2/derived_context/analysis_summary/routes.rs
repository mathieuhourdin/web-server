use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError, landscape_analysis::LandscapeAnalysis, session::Session,
};

use super::model::{AnalysisSummary, NewAnalysisSummary, NewAnalysisSummaryDto};

#[debug_handler]
pub async fn get_analysis_summaries_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(analysis_id): Path<Uuid>,
) -> Result<Json<Vec<AnalysisSummary>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, &pool)?;
    if analysis.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let summaries = AnalysisSummary::find_for_analysis(analysis_id, &pool)?;
    Ok(Json(summaries))
}

#[debug_handler]
pub async fn post_analysis_summary_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(analysis_id): Path<Uuid>,
    Json(payload): Json<NewAnalysisSummaryDto>,
) -> Result<Json<AnalysisSummary>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, &pool)?;
    if analysis.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let summary = NewAnalysisSummary::new(payload, analysis_id, user_id).create(&pool)?;
    Ok(Json(summary))
}

#[debug_handler]
pub async fn get_analysis_summary_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<AnalysisSummary>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let summary = AnalysisSummary::find(id, &pool)?;
    if summary.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(summary))
}

#[debug_handler]
pub async fn put_analysis_summary_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewAnalysisSummaryDto>,
) -> Result<Json<AnalysisSummary>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut summary = AnalysisSummary::find(id, &pool)?;
    if summary.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    summary.summary_type = payload.summary_type.unwrap_or(summary.summary_type);
    summary.title = payload.title;
    summary.content = payload.content;
    let summary = summary.update(&pool)?;
    Ok(Json(summary))
}
