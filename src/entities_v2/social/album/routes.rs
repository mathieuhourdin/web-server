use axum::{
    debug_handler,
    extract::{Extension, Json, Multipart, Path, Query},
};
use serde::Serialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    platform_infra::asset::{upload_image_asset_for_user_from_multipart, AssetUploadResponse},
    session::Session,
};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::model::{
    Album, AlbumItem, AlbumItemView, AlbumWithItems, NewAlbum, NewAlbumDto, NewAlbumItemDto,
    UpdateAlbumDto,
};

#[derive(Serialize)]
pub struct AlbumAssetUploadResponse {
    pub album: Album,
    pub asset: crate::entities_v2::asset::Asset,
    pub signed_url: String,
    pub expires_at: chrono::NaiveDateTime,
}

#[debug_handler]
pub async fn get_user_albums_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Album>>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (albums, total) = Album::find_for_user_paginated(
        viewer_user_id,
        user_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(albums, pagination, total)))
}

#[debug_handler]
pub async fn post_album_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewAlbumDto>,
) -> Result<Json<Album>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let album = Album::create(NewAlbum::new(payload, user_id), &pool)?;
    Ok(Json(album))
}

#[debug_handler]
pub async fn get_album_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<AlbumWithItems>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let album = Album::find(id, &pool)?;
    if !album.user_can_read(viewer_user_id, &pool)? {
        return Err(PpdcError::unauthorized());
    }
    let items = album.find_items_for_viewer(viewer_user_id, &pool)?;
    Ok(Json(AlbumWithItems { album, items }))
}

#[debug_handler]
pub async fn put_album_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAlbumDto>,
) -> Result<Json<Album>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut album = Album::find(id, &pool)?;
    if album.owner_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    if let Some(title) = payload.title {
        album.title = title;
    }
    if let Some(subtitle) = payload.subtitle {
        album.subtitle = subtitle;
    }
    if let Some(content) = payload.content {
        album.content = content;
    }
    if let Some(ordering_mode) = payload.ordering_mode {
        album.ordering_mode = ordering_mode;
    }
    if let Some(completion_status) = payload.completion_status {
        album.completion_status = completion_status;
    }
    if let Some(visibility) = payload.visibility {
        album.visibility = visibility;
    }
    if let Some(cover_asset_id) = payload.cover_asset_id {
        album.cover_asset_id = cover_asset_id;
    }

    Ok(Json(album.update(&pool)?))
}

#[debug_handler]
pub async fn post_album_asset_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<AlbumAssetUploadResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut album = Album::find(id, &pool)?;
    if album.owner_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let AssetUploadResponse {
        asset,
        signed_url,
        expires_at,
    } = upload_image_asset_for_user_from_multipart(user_id, &pool, multipart).await?;

    album.cover_asset_id = Some(asset.id);
    let album = album.update(&pool)?;

    Ok(Json(AlbumAssetUploadResponse {
        album,
        asset,
        signed_url,
        expires_at,
    }))
}

#[debug_handler]
pub async fn get_album_items_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(album_id): Path<Uuid>,
) -> Result<Json<Vec<AlbumItemView>>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let album = Album::find(album_id, &pool)?;
    if !album.user_can_read(viewer_user_id, &pool)? {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(album.find_items_for_viewer(viewer_user_id, &pool)?))
}

#[debug_handler]
pub async fn post_album_item_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(album_id): Path<Uuid>,
    Json(payload): Json<NewAlbumItemDto>,
) -> Result<Json<AlbumItem>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let album = Album::find(album_id, &pool)?;
    if album.owner_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let item = AlbumItem::create_or_update(
        &album,
        payload.trace_id,
        payload.ordering_index.unwrap_or(0),
        &pool,
    )?;
    Ok(Json(item))
}

#[debug_handler]
pub async fn delete_album_item_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((album_id, item_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<AlbumItem>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let album = Album::find(album_id, &pool)?;
    if album.owner_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(AlbumItem::delete(album_id, item_id, &pool)?))
}
