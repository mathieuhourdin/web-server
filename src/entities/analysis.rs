use serde::Deserialize;
use axum::{debug_handler, extract::{Json, Extension, Path}};
use crate::db::DbPool;
use crate::entities::{
    session::Session, 
    error::{PpdcError}, 
    interaction::model::{Interaction, NewInteraction, InteractionWithResource}, 
    resource::{Resource, resource_type::ResourceType, maturing_state::MaturingState, NewResource},
    resource_relation::{NewResourceRelation, ResourceRelation}
};
use uuid::Uuid;
use crate::work_analyzer::analysis_processor::analyse_to_date;
use chrono::{NaiveDate, Utc};


#[derive(Deserialize)]
pub struct NewAnalysisDto {
    pub date: Option<NaiveDate>,
    pub user_id: Uuid,
}

#[debug_handler]
pub async fn post_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewAnalysisDto>,
) -> Result<Json<Resource>, PpdcError> {
    let date = payload.date.unwrap_or_else(|| Utc::now().date_naive());

    Ok(Json(analyse_to_date(payload.user_id, date, &pool).await?))
}

#[debug_handler]
pub async fn delete_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Resource>, PpdcError> {
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
    Ok(Json(analysis))
}


pub fn find_last_analysis_resource(user_id: Uuid, pool: &DbPool) -> Result<Option<InteractionWithResource>, PpdcError> {
    let interaction = Interaction::find_last_analysis_for_user(user_id, pool)?;
    Ok(interaction)
}
