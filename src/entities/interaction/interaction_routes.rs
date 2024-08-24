use crate::http::{HttpRequest, HttpResponse};
use diesel::prelude::*;
use crate::schema::*;
use serde_json;
use super::model::*;
use uuid::Uuid;
use crate::entities::{session::Session, error::PpdcError};
use crate::pagination::PaginationParams;
use axum::{debug_handler, extract::{Extension, Path, Query, Json}};
use serde::{Deserialize};

pub fn post_interaction_for_resource(resource_id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }

    let resource_id = HttpRequest::parse_uuid(resource_id)?;

    let mut interaction = serde_json::from_str::<NewInteraction>(&request.body[..])?;

    interaction.interaction_user_id = Some(interaction.interaction_user_id.unwrap_or(request.session.as_ref().unwrap().user_id.unwrap()));
    interaction.resource_id = Some(resource_id);
    interaction.interaction_type = Some(interaction.interaction_type.unwrap_or("outp".to_string()));
    HttpResponse::ok()
        .json(&interaction.create()?)
}

#[derive(Deserialize)]
pub struct InteractionFilters {
    interaction_type: Option<String>,
    maturing_state: Option<String>,
    interaction_user_id: Option<Uuid>
}

impl InteractionFilters {
    pub fn interaction_type(&self) -> String {
        self.interaction_type.clone().unwrap_or_else(|| "inpt".into())
    }
    pub fn maturing_state(&self) -> String {
        self.maturing_state.clone().unwrap_or_else(|| "fnsh".into())
    }
}

#[debug_handler]
pub async fn get_interactions_route(
    Extension(session): Extension<Session>,
    Query(filters): Query<InteractionFilters>,
    Query(pagination): Query<PaginationParams>
) -> Result<Json<Vec<InteractionWithResource>>, PpdcError> {

    let user_id = session.user_id.unwrap();

    let interaction_type = filters.interaction_type();

    let mut interactions_filtered;
    interactions_filtered = interactions::table
        .filter(interactions::interaction_is_public.eq(true))
        .filter(interactions::interaction_type.eq(interaction_type.as_str()))
        .into_boxed();

    if 
        filters.interaction_type() == "rvew".to_string() ||
        filters.interaction_type() == "outp".to_string() &&
        filters.maturing_state() == "drft".to_string()
    {
        interactions_filtered = interactions_filtered
            .filter(interactions::interaction_user_id.eq(user_id));
    }
    if let Some(interaction_user_id) = filters.interaction_user_id {
        interactions_filtered = interactions_filtered
            .filter(interactions::interaction_user_id.eq(interaction_user_id));
    }

    let interactions = Interaction::load_paginated(
        pagination.offset(),
        pagination.limit(), 
        interactions_filtered,
        filters.maturing_state().as_str(),
        "all"

    )?;
    Ok(Json(interactions))
}
    
#[debug_handler]
pub async fn put_interaction_route(
    Path(id): Path<Uuid>,
    Json(payload): Json<NewInteraction>
) -> Result<Json<Interaction>, PpdcError> {
    Ok(Json(payload.update(&id)?))
}

pub fn get_interactions_for_resource_route(resource_id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let resource_id = HttpRequest::parse_uuid(resource_id)?;
    let limit: i64 = request.query.get("limit").unwrap_or(&"20".to_string()).parse().unwrap();
    let offset: i64 = request.query.get("offset").unwrap_or(&"0".to_string()).parse().unwrap();
    let interactions = Interaction::load_paginated(
        offset,
        limit, 
        interactions::table
            .filter(interactions::resource_id.eq(resource_id))
            .filter(interactions::interaction_is_public.eq(true)).into_boxed(),
        "fnsh",
        "all"
    )?;
    HttpResponse::ok()
        .json(&interactions)
}
