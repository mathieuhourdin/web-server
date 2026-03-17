use axum::{
    debug_handler,
    extract::{Extension, Json, Path, RawQuery},
};
use serde::Serialize;
use std::cmp::Reverse;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    analysis_summary::{AnalysisSummary, AnalysisSummaryType, MeaningfulEvent},
    error::{ErrorType, PpdcError},
    landscape_analysis::{
        model::LandscapeAnalysisType, LandscapeAnalysis, LandscapeProcessingState,
    },
    session::Session,
    trace::Trace,
};
use crate::work_analyzer;

use super::model::{Lens, NewLens, NewLensDto};
use super::persist::delete_lens_and_landscapes;

#[derive(Serialize)]
pub struct LensWeekEventAggregate {
    pub week_start: chrono::NaiveDateTime,
    pub week_end: chrono::NaiveDateTime,
    pub weekly_analysis_id: Uuid,
    pub analysis_summary_id: Option<Uuid>,
    pub meaningful_event: Option<MeaningfulEvent>,
}

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
                if payload_trace.interaction_date > current_trace.interaction_date {
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
    if let Some(next_autoplay) = payload.autoplay {
        if lens.autoplay != next_autoplay {
            lens = lens.update_autoplay(next_autoplay, &pool)?;
        }
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
    RawQuery(raw_query): RawQuery,
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

    let landscape_analysis_type = crate::pagination::parse_repeated_query_param::<
        LandscapeAnalysisType,
    >(raw_query.as_deref(), "landscape_analysis_type")?;

    let mut analyses = lens
        .get_analysis_scope(&pool)?
        .into_iter()
        .filter(|analysis| {
            analysis.processing_state == LandscapeProcessingState::Completed
                && (landscape_analysis_type.is_empty()
                    || landscape_analysis_type.contains(&analysis.landscape_analysis_type))
        })
        .collect::<Vec<_>>();
    analyses
        .sort_by_key(|analysis| Reverse(analysis.interaction_date.unwrap_or(analysis.created_at)));
    Ok(Json(analyses))
}

#[debug_handler]
pub async fn get_lens_week_events_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<LensWeekEventAggregate>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let lens = Lens::find_full_lens(id, &pool)?;
    if lens.user_id != Some(user_id) {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "Lens does not belong to current user".to_string(),
        ));
    }

    let mut weekly_analyses = lens
        .get_analysis_scope(&pool)?
        .into_iter()
        .filter(|analysis| {
            analysis.processing_state == LandscapeProcessingState::Completed
                && analysis.landscape_analysis_type == LandscapeAnalysisType::WeeklyRecap
        })
        .collect::<Vec<_>>();
    weekly_analyses.sort_by_key(|analysis| Reverse(analysis.period_start));

    let mut week_events = Vec::with_capacity(weekly_analyses.len());
    for analysis in weekly_analyses {
        let summary = AnalysisSummary::find_for_analysis(analysis.id, &pool)?
            .into_iter()
            .find(|summary| summary.summary_type == AnalysisSummaryType::PeriodRecap);

        week_events.push(LensWeekEventAggregate {
            week_start: analysis.period_start,
            week_end: analysis.period_end,
            weekly_analysis_id: analysis.id,
            analysis_summary_id: summary.as_ref().map(|summary| summary.id),
            meaningful_event: summary.and_then(|summary| summary.meaningful_event),
        });
    }

    Ok(Json(week_events))
}

#[debug_handler]
pub async fn post_lens_retry_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Lens>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let lens = Lens::find_full_lens(id, &pool)?;
    if lens.user_id != Some(user_id) {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "Lens does not belong to current user".to_string(),
        ));
    }

    let mut failed_analyses = lens
        .get_analysis_scope(&pool)?
        .into_iter()
        .filter(|analysis| analysis.processing_state == LandscapeProcessingState::Failed)
        .collect::<Vec<_>>();
    if failed_analyses.is_empty() {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "No failed analysis found for this lens".to_string(),
        ));
    }
    failed_analyses.sort_by_key(|analysis| {
        (
            analysis.period_end,
            retry_priority_for_analysis_type(analysis.landscape_analysis_type),
            analysis.created_at,
        )
    });
    let analysis_to_retry = failed_analyses
        .into_iter()
        .next()
        .expect("failed analyses not empty");

    let _ = analysis_to_retry.set_processing_state(LandscapeProcessingState::Pending, &pool)?;

    tokio::spawn(async move { work_analyzer::run_lens(lens.id).await });

    let lens = Lens::find_full_lens(id, &pool)?;
    Ok(Json(lens))
}

fn retry_priority_for_analysis_type(analysis_type: LandscapeAnalysisType) -> i32 {
    match analysis_type {
        LandscapeAnalysisType::DailyRecap => 0,
        LandscapeAnalysisType::WeeklyRecap => 1,
        _ => 0,
    }
}
