use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    landscape_analysis::LandscapeAnalysis,
    platform_infra::mailer::{self, NewOutboundEmail, OutboundEmailProvider},
    session::Session,
    trace::Trace,
    user::{User, UserPrincipalType},
};
use crate::pagination::{PaginatedResponse, PaginationParams};
use crate::work_analyzer;

use super::attachment::{MessageAttachment, MessageAttachmentType};
use super::model::{Message, MessageProcessingState, MessageType, NewMessage, NewMessageDto};

#[derive(Deserialize)]
pub struct MessageFiltersQuery {
    pub landscape_analysis_id: Option<Uuid>,
    pub received_only: Option<bool>,
    pub unread_only: Option<bool>,
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Deserialize)]
pub struct AnalysisMessagesQuery {
    pub received_only: Option<bool>,
    pub unread_only: Option<bool>,
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Serialize)]
pub struct MessageCreationResponse {
    pub question_message: Message,
    pub pending_reply_message: Option<Message>,
}

pub(crate) fn enqueue_received_message_notification_email(
    message: &Message,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let sender = User::find(&message.sender_user_id, pool)?;
    let recipient = User::find(&message.recipient_user_id, pool)?;
    if sender.principal_type != UserPrincipalType::Human
        || recipient.principal_type != UserPrincipalType::Human
        || recipient.email.trim().is_empty()
        || sender.id == recipient.id
    {
        return Ok(None);
    }

    let template = mailer::message_received_email(
        &recipient.display_name(),
        &sender.display_name(),
        &message.title,
        &message.content,
    );
    let email = NewOutboundEmail::new(
        Some(recipient.id),
        "USER_MESSAGE_RECEIVED".to_string(),
        Some("MESSAGE".to_string()),
        Some(message.id),
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
pub async fn get_messages_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(filters): Query<MessageFiltersQuery>,
) -> Result<Json<PaginatedResponse<Message>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = filters.pagination.validate()?;
    let (messages, total) = Message::find_for_participant_filtered_paginated(
        user_id,
        filters.landscape_analysis_id,
        filters.received_only.unwrap_or(false),
        filters.unread_only.unwrap_or(false),
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(messages, pagination, total)))
}

#[debug_handler]
pub async fn get_analysis_messages_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(analysis_id): Path<Uuid>,
    Query(params): Query<AnalysisMessagesQuery>,
) -> Result<Json<PaginatedResponse<Message>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, &pool)?;
    if analysis.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let pagination = params.pagination.validate()?;
    let (messages, total) = Message::find_for_participant_filtered_paginated(
        user_id,
        Some(analysis_id),
        params.received_only.unwrap_or(false),
        params.unread_only.unwrap_or(false),
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(messages, pagination, total)))
}

#[debug_handler]
pub async fn get_analysis_feedback_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(analysis_id): Path<Uuid>,
) -> Result<Json<Option<Message>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, &pool)?;
    if analysis.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let feedback = Message::find_latest_feedback_for_analysis(analysis_id, user_id, &pool)?;
    Ok(Json(feedback))
}

#[debug_handler]
pub async fn get_message_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Message>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let message = Message::find(id, &pool)?;
    if message.sender_user_id != user_id && message.recipient_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(message))
}

#[debug_handler]
pub async fn patch_message_seen_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Message>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let message = Message::find(id, &pool)?;
    let message = message.mark_seen(user_id, &pool)?;
    Ok(Json(message))
}

#[debug_handler]
pub async fn post_message_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewMessageDto>,
) -> Result<Json<MessageCreationResponse>, PpdcError> {
    let sender_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let message_type = payload.message_type.unwrap_or(MessageType::General);

    if let Some(analysis_id) = payload.landscape_analysis_id {
        let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, &pool)?;
        if analysis.user_id != payload.recipient_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Recipient must own the linked analysis".to_string(),
            ));
        }
    }

    if let Some(trace_id) = payload.trace_id {
        let trace = Trace::find_full_trace(trace_id, &pool)?;
        if trace.user_id != payload.recipient_user_id && trace.user_id != sender_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Linked trace must belong to sender or recipient".to_string(),
            ));
        }
    }
    if matches!(
        message_type,
        MessageType::Question | MessageType::TarotReadingRequest
    ) {
        if message_type == MessageType::TarotReadingRequest {
            let has_tarot_attachment = matches!(
                (payload.attachment_type, payload.attachment.as_ref()),
                (
                    Some(MessageAttachmentType::TarotReading),
                    Some(MessageAttachment::TarotReading(_))
                )
            );
            if !has_tarot_attachment {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "tarot_reading_request requires attachment_type=tarot_reading and a tarot attachment payload".to_string(),
                ));
            }
        }

        let question_message = NewMessage::new(payload, sender_user_id).create(&pool)?;
        let pending_reply_message = NewMessage {
            sender_user_id: question_message.recipient_user_id,
            recipient_user_id: sender_user_id,
            landscape_analysis_id: question_message.landscape_analysis_id,
            trace_id: question_message.trace_id,
            reply_to_message_id: Some(question_message.id),
            message_type: MessageType::MentorReply,
            processing_state: MessageProcessingState::Pending,
            title: String::new(),
            content: String::new(),
            attachment_type: None,
            attachment: None,
        }
        .create(&pool)?;

        let pool_for_task = pool.clone();
        tokio::spawn(async move {
            let _ = work_analyzer::run_message(pending_reply_message.id, &pool_for_task).await;
        });

        return Ok(Json(MessageCreationResponse {
            question_message,
            pending_reply_message: Some(pending_reply_message),
        }));
    }

    let message = NewMessage::new(payload, sender_user_id).create(&pool)?;
    if let Some(email_id) = enqueue_received_message_notification_email(&message, &pool)? {
        let pool_for_task = pool.clone();
        tokio::spawn(async move {
            let _ = mailer::process_pending_emails(vec![email_id], &pool_for_task).await;
        });
    }
    Ok(Json(MessageCreationResponse {
        question_message: message,
        pending_reply_message: None,
    }))
}

#[debug_handler]
pub async fn put_message_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewMessageDto>,
) -> Result<Json<Message>, PpdcError> {
    let sender_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut message = Message::find(id, &pool)?;
    if message.sender_user_id != sender_user_id {
        return Err(PpdcError::unauthorized());
    }

    if let Some(analysis_id) = payload.landscape_analysis_id {
        let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, &pool)?;
        if analysis.user_id != payload.recipient_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Recipient must own the linked analysis".to_string(),
            ));
        }
    }

    if let Some(trace_id) = payload.trace_id {
        let trace = Trace::find_full_trace(trace_id, &pool)?;
        if trace.user_id != payload.recipient_user_id && trace.user_id != sender_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Linked trace must belong to sender or recipient".to_string(),
            ));
        }
    }

    message.recipient_user_id = payload.recipient_user_id;
    message.landscape_analysis_id = payload.landscape_analysis_id;
    message.trace_id = payload.trace_id;
    message.message_type = payload.message_type.unwrap_or(message.message_type);
    message.title = payload.title.unwrap_or_default();
    message.content = payload.content;
    message.attachment_type = payload.attachment_type;
    message.attachment = payload.attachment;
    let message = message.update(&pool)?;
    Ok(Json(message))
}
