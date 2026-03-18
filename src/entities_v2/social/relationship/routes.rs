use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use chrono::Utc;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    platform_infra::mailer::{self, NewOutboundEmail, OutboundEmailProvider},
    session::Session,
    user::{User, UserPrincipalType, UserSearchResult},
};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::{
    enums::{RelationshipStatus, RelationshipType},
    model::{NewRelationshipDto, Relationship, UpdateRelationshipDto},
};

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

fn enqueue_follow_request_notification_email(
    relationship: &Relationship,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    if relationship.relationship_type != RelationshipType::Follow
        || relationship.status != RelationshipStatus::Pending
    {
        return Ok(None);
    }

    let requester = User::find(&relationship.requester_user_id, pool)?;
    let recipient = User::find(&relationship.target_user_id, pool)?;
    if requester.principal_type != UserPrincipalType::Human
        || recipient.principal_type != UserPrincipalType::Human
        || recipient.email.trim().is_empty()
    {
        return Ok(None);
    }

    let template = mailer::follow_request_received_email(
        &recipient.display_name(),
        &requester.display_name(),
        &requester.handle,
    );
    let email = NewOutboundEmail::new(
        Some(recipient.id),
        "FOLLOW_REQUEST_RECEIVED".to_string(),
        Some("RELATIONSHIP".to_string()),
        Some(relationship.id),
        recipient.email,
        "Matiere Grise <notifications@ppdcoeur.fr>".to_string(),
        template.subject,
        template.text_body,
        template.html_body,
        OutboundEmailProvider::Resend,
        Some(Utc::now().naive_utc()),
    )
    .create(pool)?;
    Ok(Some(email.id))
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
    Ok(Json(PaginatedResponse::new(
        relationships,
        pagination,
        total,
    )))
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
    Ok(Json(PaginatedResponse::new(
        relationships,
        pagination,
        total,
    )))
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
    Ok(Json(PaginatedResponse::new(
        relationships,
        pagination,
        total,
    )))
}

#[debug_handler]
pub async fn get_followers_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<UserSearchResult>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (relationships, total) = Relationship::find_followers_for_user_paginated(
        user_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
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
    let (relationship, should_notify) = Relationship::create_request(user_id, payload, &pool)?;
    if should_notify {
        if let Some(email_id) = enqueue_follow_request_notification_email(&relationship, &pool)? {
            let pool_for_task = pool.clone();
            tokio::spawn(async move {
                let _ = mailer::process_pending_emails(vec![email_id], &pool_for_task).await;
            });
        }
    }
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
