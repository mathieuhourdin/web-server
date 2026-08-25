use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
    http::StatusCode,
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    asset::Asset,
    error::PpdcError,
    session::Session,
    user::{User, UserSearchResult},
};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::UserBlock;

#[debug_handler]
pub async fn get_my_blocks_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<UserSearchResult>>, PpdcError> {
    let blocker_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (ids, total) = UserBlock::find_blocked_user_ids_paginated(
        blocker_user_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    let users_by_id = User::find_many(&ids, &pool)?
        .into_iter()
        .map(|user| (user.id, user))
        .collect::<std::collections::HashMap<_, _>>();
    let mut items = ids
        .into_iter()
        .filter_map(|id| users_by_id.get(&id).map(UserSearchResult::from))
        .collect::<Vec<_>>();
    let profile_picture_asset_ids = items
        .iter()
        .filter_map(|user| user.profile_picture_asset_id)
        .collect::<std::collections::HashSet<_>>();
    let public_urls_by_asset_id = Asset::find_public_urls_by_ids(
        &profile_picture_asset_ids.into_iter().collect::<Vec<_>>(),
        &pool,
    )?;
    for user in &mut items {
        user.profile_picture_display_url = user
            .profile_picture_asset_id
            .and_then(|asset_id| public_urls_by_asset_id.get(&asset_id).cloned())
            .or_else(|| user.profile_picture_url.clone());
    }
    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn put_my_block_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(blocked_user_id): Path<Uuid>,
) -> Result<StatusCode, PpdcError> {
    let blocker_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    User::find(&blocked_user_id, &pool)?;
    UserBlock::block_user(blocker_user_id, blocked_user_id, &pool)?;
    Ok(StatusCode::NO_CONTENT)
}

#[debug_handler]
pub async fn delete_my_block_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(blocked_user_id): Path<Uuid>,
) -> Result<StatusCode, PpdcError> {
    let blocker_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    UserBlock::unblock_user(blocker_user_id, blocked_user_id, &pool)?;
    Ok(StatusCode::NO_CONTENT)
}
