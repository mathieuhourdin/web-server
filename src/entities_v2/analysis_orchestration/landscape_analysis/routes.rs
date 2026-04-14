use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query, RawQuery},
    http::HeaderMap,
};
use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::HashSet;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    element::Element,
    error::{ErrorType, PpdcError},
    landmark::Landmark,
    lens::Lens,
    session::Session,
    trace::Trace,
    trace_mirror::TraceMirror,
    user::User,
};
use crate::environment;
use crate::work_analyzer;

use super::model::{
    LandscapeAnalysis, LandscapeAnalysisType, LandscapeProcessingState, NewLandscapeAnalysis,
};
use super::persist::{
    delete_leaf_and_cleanup, find_last_analysis_resource, find_lens_ids_with_pending_analyses,
};

#[derive(Deserialize)]
pub struct NewAnalysisDto {
    pub date: Option<NaiveDate>,
    pub user_id: Uuid,
}

#[derive(Serialize)]
pub struct PendingAnalysesRunError {
    pub lens_id: Uuid,
    pub message: String,
}

#[derive(Serialize)]
pub struct PendingAnalysesRunResponse {
    pub candidate_lens_ids: Vec<Uuid>,
    pub processed_lens_ids: Vec<Uuid>,
    pub failed: Vec<PendingAnalysesRunError>,
}

#[derive(Deserialize)]
pub struct LandmarksQuery {
    pub kind: Option<String>,
    pub order_by: Option<String>,
    pub order: Option<String>,
}

impl LandmarksQuery {
    pub fn relation_type(&self) -> Result<Option<&'static str>, PpdcError> {
        match self.kind.as_deref() {
            None | Some("all") => Ok(None),
            Some("mentioned") => Ok(Some("ownr")),
            Some("context") => Ok(Some("refr")),
            Some(other) => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid landmarks kind: {}", other),
            )),
        }
    }

    fn sort(&self) -> Result<(LandmarkOrderBy, SortOrder), PpdcError> {
        let order_by = match self.order_by.as_deref() {
            None | Some("related_elements_count") => LandmarkOrderBy::RelatedElementsCount,
            Some("last_related_element_at") => LandmarkOrderBy::LastRelatedElementAt,
            Some("created_at") => LandmarkOrderBy::CreatedAt,
            Some(other) => {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    format!("Invalid landmarks order_by: {}", other),
                ))
            }
        };

        let order = match self.order.as_deref() {
            None | Some("desc") => SortOrder::Desc,
            Some("asc") => SortOrder::Asc,
            Some(other) => {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    format!("Invalid landmarks order: {}", other),
                ))
            }
        };

        Ok((order_by, order))
    }
}

#[derive(Clone, Copy)]
enum LandmarkOrderBy {
    RelatedElementsCount,
    LastRelatedElementAt,
    CreatedAt,
}

#[derive(Clone, Copy)]
enum SortOrder {
    Asc,
    Desc,
}

fn sort_landmarks(landmarks: &mut [Landmark], order_by: LandmarkOrderBy, order: SortOrder) {
    match (order_by, order) {
        (LandmarkOrderBy::RelatedElementsCount, SortOrder::Desc) => landmarks.sort_by(|a, b| {
            b.related_elements_count
                .cmp(&a.related_elements_count)
                .then_with(|| b.created_at.cmp(&a.created_at))
        }),
        (LandmarkOrderBy::RelatedElementsCount, SortOrder::Asc) => landmarks.sort_by(|a, b| {
            a.related_elements_count
                .cmp(&b.related_elements_count)
                .then_with(|| a.created_at.cmp(&b.created_at))
        }),
        (LandmarkOrderBy::LastRelatedElementAt, SortOrder::Desc) => landmarks.sort_by(|a, b| {
            b.last_related_element_at
                .cmp(&a.last_related_element_at)
                .then_with(|| b.created_at.cmp(&a.created_at))
        }),
        (LandmarkOrderBy::LastRelatedElementAt, SortOrder::Asc) => landmarks.sort_by(|a, b| {
            a.last_related_element_at
                .cmp(&b.last_related_element_at)
                .then_with(|| a.created_at.cmp(&b.created_at))
        }),
        (LandmarkOrderBy::CreatedAt, SortOrder::Desc) => {
            landmarks.sort_by(|a, b| b.created_at.cmp(&a.created_at))
        }
        (LandmarkOrderBy::CreatedAt, SortOrder::Asc) => {
            landmarks.sort_by(|a, b| a.created_at.cmp(&b.created_at))
        }
    }
}

fn filtered_completed_analyses_for_lens(
    lens: Lens,
    raw_query: Option<&str>,
    pool: &DbPool,
) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
    let landscape_analysis_type =
        crate::pagination::parse_repeated_query_param::<LandscapeAnalysisType>(
            raw_query,
            "landscape_analysis_type",
        )?;

    let mut analyses = lens
        .get_analysis_scope(pool)?
        .into_iter()
        .filter(|analysis| {
            analysis.processing_state == LandscapeProcessingState::Completed
                && (landscape_analysis_type.is_empty()
                    || landscape_analysis_type.contains(&analysis.landscape_analysis_type))
        })
        .collect::<Vec<_>>();
    analyses
        .sort_by_key(|analysis| Reverse(analysis.interaction_date.unwrap_or(analysis.created_at)));
    Ok(analyses)
}

#[debug_handler]
pub async fn post_run_pending_analyses_route(
    Extension(pool): Extension<DbPool>,
    headers: HeaderMap,
) -> Result<Json<PendingAnalysesRunResponse>, PpdcError> {
    let provided_token = headers
        .get("x-internal-cron-token")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(PpdcError::unauthorized)?;

    if provided_token != environment::get_internal_cron_token() {
        return Err(PpdcError::unauthorized());
    }

    let candidate_lens_ids = find_lens_ids_with_pending_analyses(&pool)?;
    let mut processed_lens_ids = Vec::new();
    let mut failed = Vec::new();

    for lens_id in candidate_lens_ids.iter().copied() {
        match work_analyzer::run_lens(lens_id).await {
            Ok(_) => processed_lens_ids.push(lens_id),
            Err(err) => failed.push(PendingAnalysesRunError {
                lens_id,
                message: err.message,
            }),
        }
    }

    Ok(Json(PendingAnalysesRunResponse {
        candidate_lens_ids,
        processed_lens_ids,
        failed,
    }))
}

#[debug_handler]
pub async fn post_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewAnalysisDto>,
) -> Result<Json<LandscapeAnalysis>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if payload.user_id != session_user_id {
        return Err(PpdcError::unauthorized());
    }
    let date = payload.date.unwrap_or_else(|| Utc::now().date_naive());
    let user_id = session_user_id;
    let anchor_date = date.and_hms_opt(12, 0, 0).expect("valid date");

    let last_analysis = find_last_analysis_resource(user_id, &pool)?;
    let analysis_title = match &last_analysis {
        Some(last_analysis) => {
            let last_date = last_analysis
                .interaction_date
                .unwrap_or(last_analysis.created_at);
            if last_date
                > (date - Duration::days(1))
                    .and_hms_opt(12, 0, 0)
                    .expect("valid date")
            {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Analysis already exists for this day".to_string(),
                ));
            }
            format!(
                "Analyse du {} au {}",
                last_date.format("%Y-%m-%d"),
                anchor_date.format("%Y-%m-%d")
            )
        }
        None => format!(
            "Première Analyse, jusqu'au {}",
            anchor_date.format("%Y-%m-%d")
        ),
    };

    let analysis = NewLandscapeAnalysis::new(
        analysis_title,
        "Analyse".to_string(),
        "Analyse".to_string(),
        user_id,
        anchor_date,
        None,
        None,
        None,
    )
    .create(&pool)?;

    Ok(Json(analysis))
}

#[debug_handler]
pub async fn get_current_lens_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    RawQuery(raw_query): RawQuery,
) -> Result<Json<Vec<LandscapeAnalysis>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
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

    let analyses = filtered_completed_analyses_for_lens(lens, raw_query.as_deref(), &pool)?;
    Ok(Json(analyses))
}

#[debug_handler]
pub async fn delete_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<LandscapeAnalysis>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let analysis = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    if analysis.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let analysis = delete_leaf_and_cleanup(id, &pool)?;
    Ok(Json(analysis.expect("No analysis found")))
}

#[debug_handler]
pub async fn get_landmarks_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Query(params): Query<LandmarksQuery>,
) -> Result<Json<Vec<Landmark>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let landscape = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    if landscape.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let relation_type = params.relation_type()?;
    let (order_by, order) = params.sort()?;
    let mut landmarks = landscape.get_landmarks(relation_type, &pool)?;
    sort_landmarks(&mut landmarks, order_by, order);
    Ok(Json(landmarks))
}

#[debug_handler]
pub async fn get_elements_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Element>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let landscape = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    if landscape.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let elements = landscape.get_elements(&pool)?;
    Ok(Json(elements))
}

#[debug_handler]
pub async fn get_analysis_traces_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Trace>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let landscape = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    if landscape.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let inputs = landscape.get_inputs(&pool)?;

    let mut trace_ids: HashSet<Uuid> = HashSet::new();
    if let Some(trace_id) = landscape.analyzed_trace_id {
        trace_ids.insert(trace_id);
    }
    if let Some(trace_mirror_id) = landscape.trace_mirror_id {
        if let Ok(trace_mirror) = TraceMirror::find_full_trace_mirror(trace_mirror_id, &pool) {
            trace_ids.insert(trace_mirror.trace_id);
        }
    }
    for input in inputs {
        if let Some(trace_id) = input.trace_id {
            trace_ids.insert(trace_id);
        }
        if let Some(trace_mirror_id) = input.trace_mirror_id {
            if let Ok(trace_mirror) = TraceMirror::find_full_trace_mirror(trace_mirror_id, &pool) {
                trace_ids.insert(trace_mirror.trace_id);
            }
        }
    }

    let mut traces = trace_ids
        .into_iter()
        .filter_map(|trace_id| Trace::find_full_trace(trace_id, &pool).ok())
        .collect::<Vec<_>>();
    traces.sort_by_key(|trace| Reverse((trace.interaction_date, trace.created_at, trace.id)));
    Ok(Json(traces))
}

#[debug_handler]
pub async fn get_analysis_trace_mirrors_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TraceMirror>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let landscape = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    if landscape.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let inputs = landscape.get_inputs(&pool)?;

    let mut trace_mirror_ids: HashSet<Uuid> = HashSet::new();
    if let Some(trace_mirror_id) = landscape.trace_mirror_id {
        trace_mirror_ids.insert(trace_mirror_id);
    }
    for input in inputs {
        if let Some(trace_mirror_id) = input.trace_mirror_id {
            trace_mirror_ids.insert(trace_mirror_id);
        }
    }

    let mut trace_mirrors = trace_mirror_ids
        .into_iter()
        .filter_map(|trace_mirror_id| {
            TraceMirror::find_full_trace_mirror(trace_mirror_id, &pool).ok()
        })
        .collect::<Vec<_>>();
    trace_mirrors.sort_by_key(|trace_mirror| Reverse(trace_mirror.created_at));
    Ok(Json(trace_mirrors))
}

#[debug_handler]
pub async fn get_last_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<LandscapeAnalysis>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if session_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let last_analysis = find_last_analysis_resource(session_user_id, &pool)?;
    Ok(Json(last_analysis.expect("No last analysis found")))
}

#[debug_handler]
pub async fn get_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<LandscapeAnalysis>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let landscape = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    if landscape.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(landscape))
}

#[debug_handler]
pub async fn get_analysis_parents_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<LandscapeAnalysis>>, PpdcError> {
    let landscape = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if landscape.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let user = User::find(&user_id, &pool)?;

    let lens = if let Some(current_lens_id) = user.current_lens_id {
        let lens = Lens::find_full_lens(current_lens_id, &pool)?;
        if lens.user_id != Some(user_id) {
            return Err(PpdcError::unauthorized());
        }
        Some(lens)
    } else {
        let scoped_lenses = landscape.get_scoped_lenses(&pool)?;
        scoped_lenses
            .into_iter()
            .find_map(|lens| match Lens::find_full_lens(lens.id, &pool) {
                Ok(full_lens) if full_lens.user_id == Some(user_id) => Some(full_lens),
                _ => None,
            })
    };

    let Some(lens) = lens else {
        return Ok(Json(vec![]));
    };

    let reference_date = landscape.interaction_date.unwrap_or(landscape.created_at);
    let mut seen_analysis_ids = HashSet::new();
    let mut parents = lens
        .get_analysis_scope(&pool)?
        .into_iter()
        .filter(|analysis| analysis.id != landscape.id)
        .filter(|analysis| {
            let date = analysis.interaction_date.unwrap_or(analysis.created_at);
            date <= reference_date
        })
        .filter(|analysis| seen_analysis_ids.insert(analysis.id))
        .collect::<Vec<_>>();
    parents
        .sort_by_key(|analysis| Reverse(analysis.interaction_date.unwrap_or(analysis.created_at)));
    Ok(Json(parents))
}
