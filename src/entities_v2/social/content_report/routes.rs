use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use serde_json::json;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    message::Message,
    post::Post,
    post_grant::PostGrant,
    session::Session,
    source_projection::{load_source_projection_map, SourceProjectionKind},
    user::{User, UserRole},
};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::enums::ContentReportSourceKind;
use super::model::{ContentReport, NewContentReport, NewContentReportDto};

fn ensure_admin(user_id: uuid::Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    let user = User::find(&user_id, pool)?;
    if !user.has_role(UserRole::Admin, pool)? {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "Admin role required".to_string(),
        ));
    }
    Ok(())
}

#[debug_handler]
pub async fn get_admin_content_reports_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<ContentReport>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    ensure_admin(user_id, &pool)?;
    let pagination = params.validate()?;
    let (items, total) = ContentReport::find_paginated(pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn get_admin_content_report_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ContentReport>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    ensure_admin(user_id, &pool)?;
    let report = ContentReport::find(id, &pool)?;
    Ok(Json(report))
}

#[debug_handler]
pub async fn post_content_report_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewContentReportDto>,
) -> Result<Json<ContentReport>, PpdcError> {
    let reporter_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;

    let report = match (payload.reported_message_id, payload.reported_post_id) {
        (Some(message_id), None) => {
            create_message_report(message_id, reporter_user_id, payload, &pool)?
        }
        (None, Some(post_id)) => create_post_report(post_id, reporter_user_id, payload, &pool)?,
        _ => {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Provide exactly one of reported_message_id or reported_post_id".to_string(),
            ))
        }
    };

    Ok(Json(report))
}

fn create_message_report(
    message_id: uuid::Uuid,
    reporter_user_id: uuid::Uuid,
    payload: NewContentReportDto,
    pool: &DbPool,
) -> Result<ContentReport, PpdcError> {
    let message = Message::find(message_id, pool)?;
    if message.sender_user_id != reporter_user_id && message.recipient_user_id != reporter_user_id {
        return Err(PpdcError::unauthorized());
    }

    let snapshot_attachment_json = message
        .attachment
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;
    let snapshot_metadata_json = message
        .metadata
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;
    let snapshot_context_json = Some(json!({
        "message_type": message.message_type,
        "trace_id": message.trace_id,
        "post_id": message.post_id,
        "landscape_analysis_id": message.landscape_analysis_id,
        "attachment_type": message.attachment_type,
        "reply_to_message_id": message.reply_to_message_id
    }));

    NewContentReport {
        reporter_user_id,
        reported_message_id: Some(message.id),
        reported_post_id: None,
        reported_user_id: message.sender_user_id,
        reason: payload.reason,
        reporter_comment: payload.reporter_comment.unwrap_or_default(),
        snapshot_title: message.title,
        snapshot_content: message.content,
        snapshot_attachment_json,
        snapshot_metadata_json,
        snapshot_source_kind: Some(ContentReportSourceKind::Message),
        snapshot_source_id: Some(message.id),
        snapshot_context_json,
    }
    .create(pool)
}

fn create_post_report(
    post_id: uuid::Uuid,
    reporter_user_id: uuid::Uuid,
    payload: NewContentReportDto,
    pool: &DbPool,
) -> Result<ContentReport, PpdcError> {
    let post = Post::find_full(post_id, pool)?;
    if post.user_id != reporter_user_id
        && !PostGrant::user_can_read_post(&post, reporter_user_id, pool)?
    {
        return Err(PpdcError::unauthorized());
    }

    let source_ref = post.source_ref().ok_or_else(|| {
        PpdcError::new(
            409,
            ErrorType::ApiError,
            "Reported post has no source projection".to_string(),
        )
    })?;

    let mut conn = pool.get()?;
    let projections = load_source_projection_map(&[source_ref], &mut conn)?;
    let projection = projections.get(&source_ref).ok_or_else(|| {
        PpdcError::new(
            409,
            ErrorType::ApiError,
            "Unable to hydrate reported post snapshot".to_string(),
        )
    })?;

    let snapshot_source_kind = match projection.source_kind {
        SourceProjectionKind::Trace => ContentReportSourceKind::Trace,
        SourceProjectionKind::Document => ContentReportSourceKind::Document,
        SourceProjectionKind::Album => ContentReportSourceKind::Album,
    };

    let snapshot_metadata_json = Some(json!({
        "subtitle": projection.subtitle,
        "cover_image_asset_id": projection.cover_image_asset_id,
        "post_type": post.post_type,
        "audience_role": post.audience_role,
        "post_status": post.status,
        "publishing_date": post.publishing_date
    }));
    let snapshot_context_json = Some(json!({
        "post_id": post.id,
        "journal_id": projection.journal_id
    }));

    NewContentReport {
        reporter_user_id,
        reported_message_id: None,
        reported_post_id: Some(post.id),
        reported_user_id: post.user_id,
        reason: payload.reason,
        reporter_comment: payload.reporter_comment.unwrap_or_default(),
        snapshot_title: projection.title.clone(),
        snapshot_content: projection.content.clone(),
        snapshot_attachment_json: None,
        snapshot_metadata_json,
        snapshot_source_kind: Some(snapshot_source_kind),
        snapshot_source_id: Some(projection.source_id),
        snapshot_context_json,
    }
    .create(pool)
}
