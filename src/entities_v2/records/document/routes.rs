use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, session::Session};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::model::{Document, NewDocumentDto, UpdateDocumentDto};

#[debug_handler]
pub async fn get_user_documents_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Document>>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if session_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let pagination = params.validate()?;
    let (items, total) =
        Document::find_for_user_paginated(user_id, pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn get_document_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Document>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let document = Document::find_full(id, &pool)?;
    if document.owner_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(document))
}

#[debug_handler]
pub async fn post_document_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewDocumentDto>,
) -> Result<Json<Document>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let document = Document::create(payload, user_id, &pool)?;
    Ok(Json(document))
}

#[debug_handler]
pub async fn put_document_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateDocumentDto>,
) -> Result<Json<Document>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut document = Document::find_full(id, &pool)?;
    if document.owner_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    if let Some(document_role) = payload.document_role {
        document.document_role = document_role;
    }
    if let Some(document_type) = payload.document_type {
        document.document_type = document_type;
    }
    if let Some(content_source) = payload.content_source {
        document.content_source = content_source;
    }
    if let Some(title) = payload.title {
        document.title = title;
    }
    if let Some(subtitle) = payload.subtitle {
        document.subtitle = subtitle;
    }
    if let Some(description) = payload.description {
        document.description = description;
    }
    if let Some(author_name) = payload.author_name {
        document.author_name = author_name;
    }
    if let Some(content) = payload.content {
        document.content = content;
    }
    if let Some(asset_id) = payload.asset_id {
        document.asset_id = asset_id;
    }
    if let Some(external_content_url) = payload.external_content_url {
        document.external_content_url = external_content_url;
    }

    let document = document.update(&pool)?;
    Ok(Json(document))
}
