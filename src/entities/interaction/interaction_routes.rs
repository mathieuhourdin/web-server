use super::model::*;
use crate::db::DbPool;
use crate::entities::{error::PpdcError, session::Session};
use crate::pagination::PaginationParams;
use crate::schema::*;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use diesel::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

#[debug_handler]
pub async fn post_interaction_for_resource(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(mut payload): Json<NewInteraction>,
) -> Result<Json<Interaction>, PpdcError> {
    payload.interaction_user_id = Some(
        payload
            .interaction_user_id
            .unwrap_or(session.user_id.unwrap()),
    );
    payload.resource_id = Some(id);
    payload.interaction_type = Some(payload.interaction_type.unwrap_or("outp".to_string()));
    Ok(Json(payload.create(&pool)?))
}

#[derive(Deserialize)]
pub struct InteractionFilters {
    interaction_type: Option<String>,
    maturing_state: Option<String>,
    interaction_user_id: Option<Uuid>,
    resource_type: Option<String>,
}

impl InteractionFilters {
    pub fn interaction_type(&self) -> String {
        self.interaction_type
            .clone()
            .unwrap_or_else(|| "inpt".into())
    }
    pub fn maturing_state(&self) -> String {
        self.maturing_state.clone().unwrap_or_else(|| "all".into())
    }
    pub fn resource_type(&self) -> String {
        self.resource_type.clone().unwrap_or_else(|| "all".into())
    }
}

#[debug_handler]
pub async fn get_interactions_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(filters): Query<InteractionFilters>,
    Query(pagination): Query<PaginationParams>,
) -> Result<Json<Vec<InteractionWithResource>>, PpdcError> {
    let user_id = session.user_id.unwrap();

    let interaction_type = filters.interaction_type();

    let mut interactions_filtered;
    interactions_filtered = interactions::table
        .filter(interactions::interaction_type.eq(interaction_type.as_str()))
        .into_boxed();

    if filters.interaction_type() == "rvew".to_string()
        || filters.interaction_type() == "outp".to_string()
            && filters.maturing_state() == "drft".to_string()
    {
        interactions_filtered =
            interactions_filtered.filter(interactions::interaction_user_id.eq(user_id));
    }
    if let Some(interaction_user_id) = filters.interaction_user_id {
        interactions_filtered =
            interactions_filtered.filter(interactions::interaction_user_id.eq(interaction_user_id));
        if interaction_user_id != user_id {
            interactions_filtered =
                interactions_filtered.filter(interactions::interaction_is_public.eq(true));
        }
    } else {
        interactions_filtered =
            interactions_filtered.filter(interactions::interaction_is_public.eq(true));
    }

    println!("Resource type : {}", filters.resource_type());

    let interactions = Interaction::load_paginated(
        pagination.offset(),
        pagination.limit(),
        interactions_filtered,
        filters.maturing_state().as_str(),
        filters.resource_type().as_str(),
        &pool,
    )?;
    Ok(Json(interactions))
}

#[debug_handler]
pub async fn put_interaction_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewInteraction>,
) -> Result<Json<Interaction>, PpdcError> {
    Ok(Json(payload.update(&id, &pool)?))
}

#[debug_handler]
pub async fn get_interactions_for_resource_route(
    Extension(pool): Extension<DbPool>,
    Query(pagination): Query<PaginationParams>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<InteractionWithResource>>, PpdcError> {
    let interactions = Interaction::load_paginated(
        pagination.offset(),
        pagination.limit(),
        interactions::table
            .filter(interactions::resource_id.eq(id))
            .filter(interactions::interaction_is_public.eq(true))
            .into_boxed(),
        "fnsh",
        "all",
        &pool,
    )?;
    Ok(Json(interactions))
}
