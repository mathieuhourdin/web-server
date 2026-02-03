use axum::{debug_handler, extract::{Extension, Json, Path}};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{error::PpdcError, session::Session};

use super::model::TraceMirror;

#[debug_handler]
pub async fn get_trace_mirror_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<TraceMirror>, PpdcError> {
    let trace_mirror = TraceMirror::find_full_trace_mirror(id, &pool)?;
    Ok(Json(trace_mirror))
}

#[debug_handler]
pub async fn get_trace_mirrors_by_landscape_route(
    Extension(pool): Extension<DbPool>,
    Path(landscape_analysis_id): Path<Uuid>,
) -> Result<Json<Vec<TraceMirror>>, PpdcError> {
    let trace_mirrors = TraceMirror::find_by_landscape_analysis(landscape_analysis_id, &pool)?;
    Ok(Json(trace_mirrors))
}

#[debug_handler]
pub async fn get_trace_mirrors_by_trace_route(
    Extension(pool): Extension<DbPool>,
    Path(trace_id): Path<Uuid>,
) -> Result<Json<Vec<TraceMirror>>, PpdcError> {
    let trace_mirrors = TraceMirror::find_by_trace(trace_id, &pool)?;
    Ok(Json(trace_mirrors))
}

#[debug_handler]
pub async fn get_user_trace_mirrors_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<TraceMirror>>, PpdcError> {
    let trace_mirrors = TraceMirror::find_by_user(session.user_id.unwrap(), &pool)?;
    Ok(Json(trace_mirrors))
}
