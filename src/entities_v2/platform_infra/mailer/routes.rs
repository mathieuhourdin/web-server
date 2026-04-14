use axum::{
    debug_handler,
    extract::Extension,
    http::HeaderMap,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::platform_infra::mailer::{
    process_pending_email, OutboundEmail, OutboundEmailStatus,
};
use crate::environment;

const EMAIL_CRON_BATCH_LIMIT: i64 = 100;

#[derive(Serialize)]
pub struct PendingEmailsProcessError {
    pub email_id: Uuid,
    pub message: String,
}

#[derive(Serialize)]
pub struct PendingEmailsProcessResponse {
    pub candidate_email_ids: Vec<Uuid>,
    pub processed_email_ids: Vec<Uuid>,
    pub failed: Vec<PendingEmailsProcessError>,
}

#[debug_handler]
pub async fn post_process_pending_emails_route(
    Extension(pool): Extension<DbPool>,
    headers: HeaderMap,
) -> Result<Json<PendingEmailsProcessResponse>, PpdcError> {
    let provided_token = headers
        .get("x-internal-cron-token")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(PpdcError::unauthorized)?;

    if provided_token != environment::get_internal_cron_token() {
        return Err(PpdcError::unauthorized());
    }

    let claimed_emails =
        OutboundEmail::claim_due_pending(EMAIL_CRON_BATCH_LIMIT, Uuid::new_v4(), &pool)?;
    let candidate_email_ids = claimed_emails.iter().map(|email| email.id).collect::<Vec<_>>();
    let mut processed_email_ids = Vec::new();
    let mut failed = Vec::new();

    for email in claimed_emails {
        match process_pending_email(email.id, &pool).await {
            Ok(processed_email) => match processed_email.status_enum() {
                OutboundEmailStatus::Sent => processed_email_ids.push(email.id),
                OutboundEmailStatus::Failed => failed.push(PendingEmailsProcessError {
                    email_id: email.id,
                    message: processed_email
                        .last_error
                        .unwrap_or_else(|| "Email provider send failed".to_string()),
                }),
                _ => processed_email_ids.push(email.id),
            },
            Err(err) => failed.push(PendingEmailsProcessError {
                email_id: email.id,
                message: err.message,
            }),
        }
    }

    Ok(Json(PendingEmailsProcessResponse {
        candidate_email_ids,
        processed_email_ids,
        failed,
    }))
}
