use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, post::Post, session::Session};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::model::{NewPostGrantDto, PostGrant};

#[debug_handler]
pub async fn get_post_grants_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(post_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<PostGrant>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let post = Post::find_full(post_id, &pool)?;
    if post.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let pagination = params.validate()?;
    let (grants, total) =
        PostGrant::find_for_post_paginated(post_id, pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(grants, pagination, total)))
}

#[debug_handler]
pub async fn post_post_grant_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(post_id): Path<Uuid>,
    Json(payload): Json<NewPostGrantDto>,
) -> Result<Json<PostGrant>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let post = Post::find_full(post_id, &pool)?;
    let grant = PostGrant::create_or_update(&post, user_id, payload, &pool)?;
    Ok(Json(grant))
}

#[debug_handler]
pub async fn delete_post_grant_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((post_id, grant_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PostGrant>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let grant = PostGrant::revoke(post_id, grant_id, user_id, &pool)?;
    Ok(Json(grant))
}
