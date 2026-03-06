use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use std::cmp::Reverse;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    landscape_analysis::{LandscapeAnalysis, LandscapeProcessingState},
    session::Session,
    trace::Trace,
};
use crate::work_analyzer;

use super::model::{Lens, NewLens, NewLensDto};
use super::persist::delete_lens_and_landscapes;

#[debug_handler]
pub async fn post_lens_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewLensDto>,
) -> Result<Json<Lens>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let lens = NewLens::new(payload, None, user_id).create(&pool)?;
    let mut lens = Lens::find_full_lens(lens.id, &pool)?;

    if lens.target_trace_id.is_none() {
        if let Some(most_recent_trace) = Trace::get_most_recent_for_user(user_id, &pool)? {
            lens = lens.update_target_trace(Some(most_recent_trace.id), &pool)?;
        }
    }

    tokio::spawn(async move { work_analyzer::run_lens(lens.id).await });

    Ok(Json(lens))
}

#[debug_handler]
pub async fn put_lens_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewLensDto>,
) -> Result<Json<Lens>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut lens = Lens::find_full_lens(id, &pool)?;
    if lens.user_id != Some(user_id) {
        return Err(PpdcError::unauthorized());
    }

    if payload.target_trace_id != lens.target_trace_id {
        match (payload.target_trace_id, lens.target_trace_id) {
            (Some(new_target_trace_id), Some(current_target_trace_id)) => {
                let payload_trace = Trace::find_full_trace(new_target_trace_id, &pool)?;
                let current_trace = Trace::find_full_trace(current_target_trace_id, &pool)?;
                if payload_trace.interaction_date.unwrap() > current_trace.interaction_date.unwrap()
                {
                    lens = lens.update_target_trace(Some(new_target_trace_id), &pool)?;
                    tokio::spawn(async move { work_analyzer::run_lens(lens.id).await });
                }
            }
            (Some(new_target_trace_id), None) => {
                lens = lens.update_target_trace(Some(new_target_trace_id), &pool)?;
                tokio::spawn(async move { work_analyzer::run_lens(lens.id).await });
            }
            (None, Some(_)) => {
                lens = lens.update_target_trace(None, &pool)?;
            }
            (None, None) => {}
        }
    }
    if let Some(next_processing_state) = payload.processing_state {
        lens = lens.set_processing_state(next_processing_state, &pool)?;
    }
    let lens = Lens::find_full_lens(lens.id, &pool)?;
    Ok(Json(lens))
}

#[debug_handler]
pub async fn delete_lens_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Lens>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let lens = Lens::find_full_lens(id, &pool)?;
    if lens.user_id != Some(user_id) {
        return Err(PpdcError::unauthorized());
    }
    let lens = delete_lens_and_landscapes(id, &pool)?;
    Ok(Json(lens))
}

pub async fn get_user_lenses_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<Lens>>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if session_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let lenses = Lens::get_user_lenses(session_user_id, &pool)?;
    Ok(Json(lenses))
}

#[debug_handler]
pub async fn get_lens_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<LandscapeAnalysis>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let lens = Lens::find_full_lens(id, &pool)?;
    if lens.user_id != Some(user_id) {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "Lens does not belong to current user".to_string(),
        ));
    }

    let mut analyses = lens
        .get_analysis_scope(&pool)?
        .into_iter()
        .filter(|analysis| analysis.processing_state == LandscapeProcessingState::Completed)
        .collect::<Vec<_>>();
    analyses
        .sort_by_key(|analysis| Reverse(analysis.interaction_date.unwrap_or(analysis.created_at)));
    Ok(Json(analyses))
}
