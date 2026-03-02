use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use std::cmp::Reverse;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    resource::maturing_state::MaturingState,
    session::Session,
};
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::entities_v2::trace::Trace;
use crate::work_analyzer;

use super::model::{Lens, NewLens, NewLensDto};
use super::persist::delete_lens_and_landscapes;

#[debug_handler]
pub async fn post_lens_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewLensDto>,
) -> Result<Json<Lens>, PpdcError> {
    let lens = NewLens::new(payload, None, session.user_id.unwrap()).create(&pool)?;
    let lens = Lens::find_full_lens(lens.id, &pool)?;

    tokio::spawn(async move { work_analyzer::run_lens(lens.id).await });

    Ok(Json(lens))
}

#[debug_handler]
pub async fn put_lens_route(
    Extension(pool): Extension<DbPool>,
    Extension(_session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewLensDto>,
) -> Result<Json<Lens>, PpdcError> {
    let mut lens = Lens::find_full_lens(id, &pool)?;

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
    if payload.processing_state.is_some() {
        if lens.processing_state == MaturingState::Finished
            && payload.processing_state.unwrap() == MaturingState::Replay
        {
            lens = lens.set_processing_state(payload.processing_state.unwrap(), &pool)?;
            tokio::spawn(async move { work_analyzer::run_lens(lens.id).await });
        }
    }
    let lens = Lens::find_full_lens(lens.id, &pool)?;
    Ok(Json(lens))
}

#[debug_handler]
pub async fn delete_lens_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Lens>, PpdcError> {
    let lens = delete_lens_and_landscapes(id, &pool)?;
    Ok(Json(lens))
}

pub async fn get_user_lenses_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<Lens>>, PpdcError> {
    let lenses = Lens::get_user_lenses(session.user_id.unwrap(), &pool)?;
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

    let mut analyses = lens.get_analysis_scope(&pool)?;
    analyses.sort_by_key(|analysis| Reverse(analysis.interaction_date.unwrap_or(analysis.created_at)));
    Ok(Json(analyses))
}
