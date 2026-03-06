use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError, landscape_analysis::LandscapeAnalysis, session::Session, trace::Trace,
};

use super::model::TraceMirror;

#[debug_handler]
pub async fn get_trace_mirror_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<TraceMirror>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace_mirror = TraceMirror::find_full_trace_mirror(id, &pool)?;
    if trace_mirror.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(trace_mirror))
}

#[debug_handler]
pub async fn get_trace_mirrors_by_landscape_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(landscape_analysis_id): Path<Uuid>,
) -> Result<Json<Vec<TraceMirror>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let landscape = LandscapeAnalysis::find_full_analysis(landscape_analysis_id, &pool)?;
    if landscape.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let trace_mirrors = TraceMirror::find_by_landscape_analysis(landscape_analysis_id, &pool)?;
    Ok(Json(trace_mirrors))
}

#[debug_handler]
pub async fn get_trace_mirrors_by_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
) -> Result<Json<Vec<TraceMirror>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let trace_mirrors = TraceMirror::find_by_trace(trace_id, &pool)?;
    Ok(Json(trace_mirrors))
}

#[debug_handler]
pub async fn get_user_trace_mirrors_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<TraceMirror>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace_mirrors = TraceMirror::find_by_user(user_id, &pool)?;
    Ok(Json(trace_mirrors))
}
