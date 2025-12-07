use serde::{Serialize, Deserialize};
use axum::{debug_handler, extract::{Query, Json, Path, Extension}};
use crate::entities::process_audio::qualify_user_input;
use crate::db::DbPool;
use crate::entities::{session::Session, error::{PpdcError}, user::User, interaction::model::{Interaction, NewInteraction}, resource::{NewResource, Resource, ResourceType}};


#[derive(Deserialize)]
pub struct NewTraceDto {
    pub content: String
}

impl NewTraceDto {
    async fn to_new_resource(self) -> NewResource {
        let Self {
            content,
        } = self;
        let resource_properties = qualify_user_input::extract_user_input_resource_info_with_gpt(content.as_str()).await.unwrap(); 
        let title = resource_properties.title;
        let subtitle = resource_properties.subtitle;
        NewResource::new(title, subtitle, content, ResourceType::Trace)
    }
}


#[debug_handler]
pub async fn post_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(mut payload): Json<NewTraceDto>,
) -> Result<Json<Resource>, PpdcError> {
    let new_resource = payload.to_new_resource().await;
    let resource = new_resource.create(&pool)?;
    let interaction = NewInteraction::create_instant(session.user_id.unwrap(), resource.id, &pool);

    Ok(Json(resource))
}
