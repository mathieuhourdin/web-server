use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::{Interaction, NewInteraction},
    resource::Resource,
    resource_relation::NewResourceRelation,
    session::Session,
};
use crate::work_analyzer::trace_broker::qualify_trace;
use axum::{
    debug_handler,
    extract::{Extension, Json},
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct NewTraceDto {
    pub content: String,
    pub journal_id: Uuid,
}

#[debug_handler]
pub async fn post_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewTraceDto>,
) -> Result<Json<Resource>, PpdcError> {
    let journal_interaction =
        Interaction::find_user_journal(session.user_id.unwrap(), payload.journal_id, &pool)?;

    let (new_resource, interaction_date) = qualify_trace(payload.content.as_str()).await?;

    let resource = new_resource.create(&pool)?;
    let mut new_interaction = NewInteraction::new(session.user_id.unwrap(), resource.id);

    if let Some(interaction_date) = interaction_date {
        new_interaction.interaction_date = Some(interaction_date.and_hms_opt(12, 0, 0).unwrap());
    }
    new_interaction.create(&pool)?;

    let mut new_resource_relation =
        NewResourceRelation::new(resource.id, journal_interaction.resource_id.unwrap());
    new_resource_relation.user_id = session.user_id;
    new_resource_relation.relation_type = Some("jrit".to_string());
    new_resource_relation.create(&pool)?;

    //let element_resources = create_element_resources_from_trace(resource.clone(), session.user_id.unwrap(), &pool).await?;

    Ok(Json(resource))
}
