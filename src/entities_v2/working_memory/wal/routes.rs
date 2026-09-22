use axum::{debug_handler, extract::Extension, extract::Path, extract::Query, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, session::Session, user::User};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::model::{
    WalCarryoverResponse, WalCompilationViews, WalDay, WalDayResponse, WalProjection, WalResponse,
};
use super::service::{
    append_today, apply_today_carryover, compile_today, get_or_create_today, get_today_carryover,
    get_today_response,
};

#[derive(Debug, Deserialize)]
pub struct AppendWalDto {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ApplyWalCarryoverDto {
    pub item_ids: Vec<Uuid>,
}

fn current_user(session: &Session, pool: &DbPool) -> Result<User, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    User::find(&user_id, pool)
}

#[debug_handler]
pub async fn get_wal_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<WalResponse>, PpdcError> {
    let user = current_user(&session, &pool)?;
    Ok(Json(get_today_response(&user, &pool)?))
}

#[debug_handler]
pub async fn post_wal_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<AppendWalDto>,
) -> Result<Json<WalResponse>, PpdcError> {
    if payload.content.trim().is_empty() {
        return Err(PpdcError::new(
            400,
            crate::entities_v2::error::ErrorType::ApiError,
            "Cannot append an empty WAL entry".to_string(),
        ));
    }
    let user = current_user(&session, &pool)?;
    Ok(Json(append_today(&user, payload.content, &pool)?))
}

#[debug_handler]
pub async fn get_wals_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<WalDayResponse>>, PpdcError> {
    let user = current_user(&session, &pool)?;
    let pagination = params.validate()?;
    get_or_create_today(&user, &pool)?;
    let (items, total) =
        WalDay::find_for_user_paginated(user.id, pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(
        items.into_iter().map(WalDayResponse::from).collect(),
        pagination,
        total,
    )))
}

#[debug_handler]
pub async fn get_wal_compilation_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<WalCompilationViews>, PpdcError> {
    let user = current_user(&session, &pool)?;
    let wal = get_or_create_today(&user, &pool)?;
    Ok(Json(WalProjection::compilation_views(&wal, &pool)?))
}

#[debug_handler]
pub async fn post_wal_compilation_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<WalCompilationViews>, PpdcError> {
    let user = current_user(&session, &pool)?;
    Ok(Json(compile_today(&user, session.id, &pool).await?))
}

#[debug_handler]
pub async fn get_wal_carryover_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<WalCarryoverResponse>, PpdcError> {
    let user = current_user(&session, &pool)?;
    Ok(Json(get_today_carryover(&user, &pool)?))
}

#[debug_handler]
pub async fn post_apply_wal_carryover_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(projection_id): Path<Uuid>,
    Json(payload): Json<ApplyWalCarryoverDto>,
) -> Result<Json<WalResponse>, PpdcError> {
    if payload.item_ids.is_empty() {
        return Err(PpdcError::new(
            400,
            crate::entities_v2::error::ErrorType::ApiError,
            "At least one carryover item ID is required".to_string(),
        ));
    }
    let user = current_user(&session, &pool)?;
    Ok(Json(apply_today_carryover(
        &user,
        projection_id,
        &payload.item_ids,
        &pool,
    )?))
}
