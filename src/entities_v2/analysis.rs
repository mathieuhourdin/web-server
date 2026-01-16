use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    interaction::model::{Interaction, InteractionWithResource, NewInteraction},
    resource::{maturing_state::MaturingState, resource_type::ResourceType, NewResource, Resource},
    resource_relation::ResourceRelation,
    session::Session,
};
use crate::work_analyzer::analysis_processor::run_analysis_pipeline;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

pub struct Analysis {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub user_id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize)]
pub struct NewAnalysisDto {
    pub date: Option<NaiveDate>,
    pub user_id: Uuid,
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
                > (date - Duration::days(1)).and_hms_opt(12, 0, 0).expect("Date should be valid")
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

    tokio::spawn(async move { run_analysis_pipeline(analysis_resource.id).await });

    //let resources = process_analysis(payload.user_id, date, &analysis_resource.id, last_analysis_id_option, &pool).await?;

    Ok(Json(analysis_resource))
}

#[debug_handler]
pub async fn delete_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(_session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Resource>, PpdcError> {
    let analysis = delete_analysis_resources_and_clean_graph(id, &pool)?;
    Ok(Json(analysis))
}

#[debug_handler]    
pub async fn get_last_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<InteractionWithResource>, PpdcError> {
    println!("Getting last analysis for user: {:?}", session.user_id.unwrap());
    let last_analysis = find_last_analysis_resource(session.user_id.unwrap(), &pool)?;
    Ok(Json(last_analysis.expect("No last analysis found")))
}

pub fn delete_analysis_resources_and_clean_graph(
    id: Uuid,
    pool: &DbPool,
) -> Result<Resource, PpdcError> {
    let analysis = Resource::find(id, &pool)?;
    println!("Analysis: {:?}", analysis);
    let related_resources = ResourceRelation::find_origin_for_resource(id, &pool)?;
    for resource_relation in related_resources {
        println!("Resource relation: {:?}", resource_relation);
        // check if the resource is owned by the analysis (for elements and entities)
        if resource_relation.resource_relation.relation_type == "ownr".to_string() {
            println!("Deleting resource: {:?}", resource_relation.origin_resource);
            resource_relation.origin_resource.delete(&pool)?;
        } else if resource_relation.origin_resource.resource_type == ResourceType::Trace {
            let mut trace = resource_relation.origin_resource;
            trace.maturing_state = MaturingState::Draft;
            trace.update(&pool)?;
        }
    }
    let analysis = analysis.delete(&pool)?;
    Ok(analysis)
}

pub fn find_last_analysis_resource(
    user_id: Uuid,
    pool: &DbPool,
) -> Result<Option<InteractionWithResource>, PpdcError> {
    let interaction = Interaction::find_last_analysis_for_user(user_id, pool)?;
    Ok(interaction)
}
