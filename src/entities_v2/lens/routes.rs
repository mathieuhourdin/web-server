use axum::{debug_handler, extract::{Extension, Json, Path}};
use chrono::Utc;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{error::PpdcError, session::Session};
use crate::entities_v2::landscape_analysis::NewLandscapeAnalysis;
use crate::work_analyzer;

use super::model::{Lens, NewLens, NewLensDto};
use super::persist::{create_landscape_placeholders, delete_lens_and_landscapes};

#[debug_handler]
pub async fn post_lens_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewLensDto>,
) -> Result<Json<Lens>, PpdcError> {
    let current_trace_id = payload.current_trace_id;
    let previous_landscape_analysis_id = match payload.fork_landscape_id {
        Some(fork_landscape_id) => {
            fork_landscape_id
        }
        None => {
            let initial_landscape = NewLandscapeAnalysis::new(
                "Analyse de la biographie".to_string(),
                "Analyse de la biographie".to_string(),
                "Analyse de la biographie".to_string(),
                session.user_id.unwrap(),
                Utc::now().naive_utc(),
                None,
                None,
            ).create(&pool)?;
            initial_landscape.id
        }
    };
    let landscape_analysis_ids = create_landscape_placeholders(current_trace_id, previous_landscape_analysis_id, &pool)?;
    let last_landscape_analysis_id = landscape_analysis_ids.last().unwrap();

    // after we have the first landscape, we can create the landscape placeholders from the first one using traces as diff.
    let lens = NewLens::new(
        payload, current_trace_id, 
        last_landscape_analysis_id.clone(), 
        session.user_id.unwrap()
    )
        .create(&pool)?;
    let lens = lens.with_user_id(&pool)?;
    let lens = lens.with_forked_landscape(&pool)?;
    let lens = lens.with_current_landscape(&pool)?;
    let lens = lens.with_current_trace(&pool)?;

    tokio::spawn(async move { work_analyzer::analysis_processor::run_analysis_pipeline_for_landscapes(landscape_analysis_ids).await });

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

    if payload.current_trace_id != lens.current_trace_id {
        let landscape_analysis_ids = create_landscape_placeholders(payload.current_trace_id, lens.current_landscape_id.unwrap(), &pool)?;
        let last_landscape_analysis_id = landscape_analysis_ids.last().unwrap().clone();
        lens = lens.update_current_landscape(last_landscape_analysis_id, &pool)?;
        tokio::spawn(async move { work_analyzer::analysis_processor::run_analysis_pipeline_for_landscapes(landscape_analysis_ids).await });
    }
    let lens = lens.with_user_id(&pool)?;
    let lens = lens.with_forked_landscape(&pool)?;
    let lens = lens.with_current_landscape(&pool)?;
    let lens = lens.with_current_trace(&pool)?;
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
