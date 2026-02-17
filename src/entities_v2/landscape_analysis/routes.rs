use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use chrono::{Duration, NaiveDate, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    interaction::model::{InteractionWithResource, NewInteraction},
    resource::{maturing_state::MaturingState, resource_type::ResourceType, NewResource, Resource},
    session::Session,
};
use crate::entities_v2::{element::Element, landmark::Landmark};

use super::model::LandscapeAnalysis;
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
    Extension(_session): Extension<Session>,
    Json(payload): Json<NewAnalysisDto>,
) -> Result<Json<Resource>, PpdcError> {
    let date = payload.date.unwrap_or_else(|| Utc::now().date_naive());
    let user_id = payload.user_id;

    // we find the last analysis for the user
    let last_analysis = find_last_analysis_resource(user_id, &pool)?;

    let analysis_title = match &last_analysis {
        Some(last_analysis) => {
            if last_analysis.interaction.interaction_date.clone()
                > (date - Duration::days(1))
                    .and_hms_opt(12, 0, 0)
                    .expect("Date should be valid")
            {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Analysis already exists for this day".to_string(),
                ));
            } else {
                format!(
                    "Analyse du {} au {}",
                    last_analysis
                        .interaction
                        .interaction_date
                        .clone()
                        .format("%Y-%m-%d")
                        .to_string(),
                    date.and_hms_opt(12, 0, 0)
                        .expect("Date should be valid")
                        .format("%Y-%m-%d")
                        .to_string()
                )
            }
        }
        None => {
            format!(
                "Première Analyse, jusqu'au {}",
                date.and_hms_opt(12, 0, 0)
                    .expect("Date should be valid")
                    .format("%Y-%m-%d")
                    .to_string()
            )
        }
    };
    // we create a new analysis resource that will be the support of all performed analysis in this analysis session
    let mut analysis_resource = NewResource::new(
        analysis_title,
        "Analyse".to_string(),
        "Analyse".to_string(),
        ResourceType::Analysis,
    );
    analysis_resource.maturing_state = Some(MaturingState::Draft);

    let analysis_resource = analysis_resource.create(&pool)?;

    // we create a new interaction for the analysis resource, with the given date
    let mut new_interaction = NewInteraction::new(user_id, analysis_resource.id);
    new_interaction.interaction_type = Some("outp".to_string());
    new_interaction.interaction_date = Some(date.and_hms_opt(12, 0, 0).unwrap());
    new_interaction.interaction_progress = 0;
    new_interaction.create(&pool)?;

    Ok(Json(analysis_resource))
}

#[debug_handler]
pub async fn delete_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(_session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<LandscapeAnalysis>, PpdcError> {
    let analysis = delete_leaf_and_cleanup(id, &pool)?;
    Ok(Json(analysis.expect("No analysis found")))
}

#[debug_handler]
pub async fn get_landmarks_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
    Query(params): Query<LandmarksQuery>,
) -> Result<Json<Vec<Landmark>>, PpdcError> {
    let landscape = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    let relation_type = params.relation_type()?;
    let landmarks = landscape.get_landmarks(relation_type, &pool)?;
    Ok(Json(landmarks))
}

#[debug_handler]
pub async fn get_elements_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Element>>, PpdcError> {
    let landscape = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    let elements = landscape.get_elements(&pool)?;
    Ok(Json(elements))
}

#[debug_handler]
pub async fn get_last_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<InteractionWithResource>, PpdcError> {
    println!(
        "Getting last analysis for user: {:?}",
        session.user_id.unwrap()
    );
    let last_analysis = find_last_analysis_resource(session.user_id.unwrap(), &pool)?;
    Ok(Json(last_analysis.expect("No last analysis found")))
}

#[debug_handler]
pub async fn get_analysis_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<LandscapeAnalysis>, PpdcError> {
    let landscape = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    Ok(Json(landscape))
}

#[debug_handler]
pub async fn get_analysis_parents_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<LandscapeAnalysis>>, PpdcError> {
    let landscape = LandscapeAnalysis::find_full_analysis(id, &pool)?;
    let parents = landscape.find_all_parents(&pool)?;
    Ok(Json(parents))
}
