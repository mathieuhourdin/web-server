use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use std::cmp::Reverse;
use std::collections::HashSet;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    session::Session,
    user::User,
};
use crate::entities_v2::journal::Journal;
use crate::entities_v2::lens::Lens;
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::work_analyzer;

use super::{
    llm_qualify,
    model::{NewTrace, NewTraceDto, Trace},
};

#[debug_handler]
pub async fn post_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewTraceDto>,
) -> Result<Json<Trace>, PpdcError> {
    let user_id = session.user_id.unwrap();
    let journal = Journal::find_full(payload.journal_id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let qualified = llm_qualify::qualify_trace(payload.content.as_str()).await?;

    let interaction_date = qualified
        .interaction_date
        .and_then(|d| d.and_hms_opt(12, 0, 0));

    let trace = NewTrace::new(
        qualified.title,
        qualified.subtitle,
        payload.content,
        interaction_date,
        user_id,
        journal.id,
    )
    .create(&pool)?;

    let user_lenses = Lens::get_user_lenses(user_id, &pool)?;
    for lens in user_lenses.into_iter().filter(|lens| lens.autoplay) {
        let lens = if lens.target_trace_id != Some(trace.id) {
            lens.update_target_trace(Some(trace.id), &pool)?
        } else {
            lens
        };
        tokio::spawn(async move { work_analyzer::run_lens(lens.id).await });
    }

    Ok(Json(trace))
}

#[debug_handler]
pub async fn get_all_traces_for_user_route(
    Extension(pool): Extension<DbPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<Trace>>, PpdcError> {
    let traces = Trace::get_all_for_user(user_id, &pool)?;
    Ok(Json(traces))
}

#[debug_handler]
pub async fn get_trace_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Trace>, PpdcError> {
    let trace = Trace::find_full_trace(id, &pool)?;
    Ok(Json(trace))
}

#[debug_handler]
pub async fn get_trace_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
) -> Result<Json<LandscapeAnalysis>, PpdcError> {
    let user_id = session.user_id.unwrap();
    let user = User::find(&user_id, &pool)?;
    let current_lens_id = user.current_lens_id.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "current_lens_id is not set for user".to_string(),
        )
    })?;

    let lens = Lens::find_full_lens(current_lens_id, &pool)?;
    if lens.user_id != Some(user_id) {
        return Err(PpdcError::unauthorized());
    }
    let analysis_scope_ids = lens
        .get_analysis_scope_ids(&pool)?
        .into_iter()
        .collect::<HashSet<_>>();

    let mut analyses = LandscapeAnalysis::find_by_trace(trace_id, &pool)?
        .into_iter()
        .filter(|analysis| analysis_scope_ids.contains(&analysis.id))
        .collect::<Vec<_>>();
    analyses.sort_by_key(|analysis| Reverse(analysis.interaction_date.unwrap_or(analysis.created_at)));

    let analysis = analyses.into_iter().next().ok_or_else(|| {
        PpdcError::new(
            404,
            ErrorType::ApiError,
            "No analysis found for trace in current lens scope".to_string(),
        )
    })?;
    Ok(Json(analysis))
}

#[debug_handler]
pub async fn get_traces_for_journal_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Trace>>, PpdcError> {
    let traces = Trace::get_all_for_journal(id, &pool)?;
    Ok(Json(traces))
}
