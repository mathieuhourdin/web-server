use axum::{
    debug_handler,
    extract::{Extension, Query},
    Json,
};

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, session::Session};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::{hydrate::find_feed_items_paginated, model::FeedItem};

#[derive(serde::Deserialize)]
pub struct FeedQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub seen: Option<bool>,
}

#[debug_handler]
pub async fn get_feed_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<FeedQuery>,
) -> Result<Json<PaginatedResponse<FeedItem>>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.pagination.validate()?;
    let (items, total) = find_feed_items_paginated(
        viewer_user_id,
        params.seen,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}
