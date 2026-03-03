use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    session::Session,
    shared::MaturingState,
};
use crate::pagination::PaginationParams;
use serde::Deserialize;

use super::model::{NewPost, NewPostDto, Post, PostInteractionType, PostType};

#[derive(Deserialize)]
pub struct PostFiltersQuery {
    pub interaction_type: Option<PostInteractionType>,
    pub post_type: Option<PostType>,
    pub is_external: Option<bool>,
    pub resource_type: Option<String>, // Legacy fallback mapped to post_type
    pub user_id: Option<Uuid>,
    pub maturing_state: Option<MaturingState>,
    pub limit: Option<i64>,
}

#[debug_handler]
pub async fn get_posts_route(
    Extension(pool): Extension<DbPool>,
    Query(filters): Query<PostFiltersQuery>,
) -> Result<Json<Vec<Post>>, PpdcError> {
    let posts = Post::find_filtered(
        filters.interaction_type,
        filters.post_type,
        filters.resource_type,
        filters.is_external,
        filters.user_id,
        filters.maturing_state,
        filters.limit.unwrap_or(20),
        &pool,
    )?;
    Ok(Json(posts))
}

#[debug_handler]
pub async fn get_post_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Post>, PpdcError> {
    let post = Post::find_full(id, &pool)?;
    Ok(Json(post))
}

#[debug_handler]
pub async fn get_user_posts_route(
    Extension(pool): Extension<DbPool>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<Post>>, PpdcError> {
    let posts = Post::find_for_user(user_id, params.offset(), params.limit(), &pool)?;
    Ok(Json(posts))
}

#[debug_handler]
pub async fn post_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewPostDto>,
) -> Result<Json<Post>, PpdcError> {
    let new_post = NewPost::new(payload, session.user_id.unwrap());
    let post = new_post.create(&pool)?;
    Ok(Json(post))
}

#[debug_handler]
pub async fn put_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewPostDto>,
) -> Result<Json<Post>, PpdcError> {
    let mut post = Post::find_full(id, &pool)?;
    if post.user_id != session.user_id.unwrap() {
        return Err(PpdcError::unauthorized());
    }

    post.title = payload.title;
    post.subtitle = payload.subtitle.unwrap_or_default();
    post.content = payload.content;
    post.image_url = payload.image_url;
    if let Some(interaction_type) = payload.interaction_type {
        post.interaction_type = interaction_type;
    }
    if let Some(post_type) = payload.post_type {
        post.post_type = post_type;
    }
    if let Some(publishing_state) = payload.publishing_state {
        post.publishing_state = publishing_state;
    }
    if let Some(maturing_state) = payload.maturing_state {
        post.maturing_state = maturing_state;
    }
    let post = post.update(&pool)?;
    Ok(Json(post))
}
