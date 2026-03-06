use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use chrono::{Duration, NaiveDate, Utc};
use serde::Deserialize;
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

use super::model::{LandscapeAnalysis, NewLandscapeAnalysis};
use super::persist::{delete_leaf_and_cleanup, find_last_analysis_resource};

#[derive(Deserialize)]
pub struct NewAnalysisDto {
    pub date: Option<NaiveDate>,
    pub user_id: Uuid,
}

#[derive(Deserialize)]
pub struct LandmarksQuery {
    pub kind: Option<String>,
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
    let landmarks = landscape.get_landmarks(relation_type, &pool)?;
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
    traces.sort_by_key(|trace| (trace.interaction_date, trace.id));
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
        .filter_map(|trace_mirror_id| TraceMirror::find_full_trace_mirror(trace_mirror_id, &pool).ok())
        .collect::<Vec<_>>();
    trace_mirrors.sort_by_key(|trace_mirror| trace_mirror.created_at);
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
