use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    session::Session,
    user::{User, UserSearchResult},
};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::model::{NewRelationshipDto, Relationship, UpdateRelationshipDto};

fn hydrate_user_results(ids: Vec<Uuid>, pool: &DbPool) -> Result<Vec<UserSearchResult>, PpdcError> {
    let users = User::find_many(&ids, pool)?;
    let users_by_id = users
        .into_iter()
        .map(|user| (user.id, user))
        .collect::<std::collections::HashMap<Uuid, User>>();

    Ok(ids
        .into_iter()
        .filter_map(|id| users_by_id.get(&id).map(UserSearchResult::from))
        .collect())
}

#[debug_handler]
pub async fn get_relationships_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Relationship>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (relationships, total) =
        Relationship::find_for_user_paginated(user_id, pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(relationships, pagination, total)))
}

#[debug_handler]
pub async fn get_incoming_relationship_requests_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Relationship>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (relationships, total) = Relationship::find_incoming_pending_for_user_paginated(
        user_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(relationships, pagination, total)))
}

#[debug_handler]
pub async fn get_outgoing_relationship_requests_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Relationship>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (relationships, total) = Relationship::find_outgoing_pending_for_user_paginated(
        user_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(relationships, pagination, total)))
}

#[debug_handler]
pub async fn get_followers_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<UserSearchResult>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (relationships, total) =
        Relationship::find_followers_for_user_paginated(user_id, pagination.offset, pagination.limit, &pool)?;
    let follower_ids = relationships
        .into_iter()
        .map(|relationship| relationship.requester_user_id)
        .collect();
    Ok(Json(PaginatedResponse::new(
        hydrate_user_results(follower_ids, &pool)?,
        pagination,
        total,
    )))
}

#[debug_handler]
pub async fn get_following_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<UserSearchResult>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (relationships, total) = Relationship::find_following_for_user_paginated(
        user_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    let following_ids = relationships
        .into_iter()
        .map(|relationship| relationship.target_user_id)
        .collect();
    Ok(Json(PaginatedResponse::new(
        hydrate_user_results(following_ids, &pool)?,
        pagination,
        total,
    )))
}

#[debug_handler]
pub async fn post_relationship_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewRelationshipDto>,
) -> Result<Json<Relationship>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let relationship = Relationship::create_request(user_id, payload, &pool)?;
    Ok(Json(relationship))
}

#[debug_handler]
pub async fn put_relationship_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRelationshipDto>,
) -> Result<Json<Relationship>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let relationship = Relationship::update_status(id, user_id, payload.status, &pool)?;
    Ok(Json(relationship))
}
