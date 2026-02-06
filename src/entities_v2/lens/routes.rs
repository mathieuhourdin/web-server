use axum::{debug_handler, extract::{Extension, Json, Path}};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{error::PpdcError, session::Session, resource::maturing_state::MaturingState};
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
    let lens = NewLens::new(
        payload, 
        None,
        session.user_id.unwrap()
    )
        .create(&pool)?;
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
        let payload_trace = Trace::find_full_trace(payload.target_trace_id, &pool)?;
        let current_trace = Trace::find_full_trace(lens.target_trace_id, &pool)?;
        if payload_trace.interaction_date.unwrap() > current_trace.interaction_date.unwrap() {
            lens = lens.update_target_trace(payload.target_trace_id, &pool)?;
            tokio::spawn(async move { work_analyzer::run_lens(lens.id).await });
        }
    }
    if payload.processing_state.is_some() {
        if lens.processing_state == MaturingState::Finished && payload.processing_state.unwrap() == MaturingState::Replay {
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
