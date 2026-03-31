use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use chrono::Utc;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    journal::Journal,
    platform_infra::mailer::{self, NewOutboundEmail, OutboundEmailProvider},
    session::Session,
    user::{User, UserPrincipalType},
};
use crate::environment;
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::model::{JournalGrant, NewJournalGrantDto};

fn enqueue_journal_access_granted_email(
    journal: &Journal,
    grant: &JournalGrant,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let Some(recipient_user_id) = grant.grantee_user_id else {
        return Ok(None);
    };

    let owner = User::find(&journal.user_id, pool)?;
    let recipient = User::find(&recipient_user_id, pool)?;
    if owner.id == recipient.id
        || owner.principal_type != UserPrincipalType::Human
        || recipient.principal_type != UserPrincipalType::Human
        || recipient.email.trim().is_empty()
    {
        return Ok(None);
    }

    let journal_url = format!(
        "{}/me/journals/{}",
        environment::get_app_base_url().trim_end_matches('/'),
        journal.id
    );
    let template = mailer::journal_access_granted_email(
        &recipient.display_name(),
        &owner.display_name(),
        &journal.title,
        &journal_url,
    );
    let email = NewOutboundEmail::new(
        Some(recipient.id),
        "JOURNAL_ACCESS_GRANTED".to_string(),
        Some("JOURNAL".to_string()),
        Some(journal.id),
        recipient.email,
        "Matiere Grise <noreply@ppdcoeur.fr>".to_string(),
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
pub async fn get_journal_grants_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<JournalGrant>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let pagination = params.validate()?;
    let (grants, total) = JournalGrant::find_for_journal_paginated(
        journal_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(grants, pagination, total)))
}

#[debug_handler]
pub async fn post_journal_grant_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    Json(payload): Json<NewJournalGrantDto>,
) -> Result<Json<JournalGrant>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    let (grant, should_notify) = JournalGrant::create_or_update(&journal, user_id, payload, &pool)?;
    if should_notify {
        if let Some(email_id) = enqueue_journal_access_granted_email(&journal, &grant, &pool)? {
            let pool_for_task = pool.clone();
            tokio::spawn(async move {
                let _ = mailer::process_pending_emails(vec![email_id], &pool_for_task).await;
            });
        }
    }
    Ok(Json(grant))
}

#[debug_handler]
pub async fn delete_journal_grant_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((journal_id, grant_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<JournalGrant>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let grant = JournalGrant::revoke(journal_id, grant_id, user_id, &pool)?;
    Ok(Json(grant))
}
