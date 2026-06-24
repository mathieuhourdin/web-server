use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, journal::Journal, session::Session};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::model::{
    JournalSharingPolicy, JournalSharingPolicyHistoryDecisionDto,
    JournalSharingPolicyPendingReview, NewJournalSharingPolicyDto, UpdateJournalSharingPolicyDto,
};

#[debug_handler]
pub async fn get_journal_sharing_policy_pending_reviews_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<JournalSharingPolicyPendingReview>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (items, total) = JournalSharingPolicy::find_pending_reviews_for_owner_paginated(
        user_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn get_journal_sharing_policies_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<JournalSharingPolicy>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let pagination = params.validate()?;
    let (policies, total) = JournalSharingPolicy::find_for_journal_paginated(
        journal_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(policies, pagination, total)))
}

#[debug_handler]
pub async fn post_journal_sharing_policy_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    Json(payload): Json<NewJournalSharingPolicyDto>,
) -> Result<Json<JournalSharingPolicy>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    let policy = JournalSharingPolicy::create_or_update(&journal, user_id, payload, &pool)?;
    Ok(Json(policy))
}

#[debug_handler]
pub async fn patch_journal_sharing_policy_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((journal_id, policy_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateJournalSharingPolicyDto>,
) -> Result<Json<JournalSharingPolicy>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let policy = JournalSharingPolicy::update(journal_id, policy_id, user_id, payload, &pool)?;
    Ok(Json(policy))
}

#[debug_handler]
pub async fn delete_journal_sharing_policy_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((journal_id, policy_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<JournalSharingPolicy>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let policy = JournalSharingPolicy::revoke(journal_id, policy_id, user_id, &pool)?;
    Ok(Json(policy))
}

#[debug_handler]
pub async fn post_journal_sharing_policy_history_decision_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((journal_id, policy_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<JournalSharingPolicyHistoryDecisionDto>,
) -> Result<Json<JournalSharingPolicy>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let policy = JournalSharingPolicy::apply_history_decision(
        journal_id, policy_id, user_id, payload, &pool,
    )?;
    Ok(Json(policy))
}
