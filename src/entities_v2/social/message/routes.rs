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
    post::Post,
    post_grant::PostGrant,
    session::Session,
    trace::Trace,
    user::{User, UserPrincipalType, UserRole},
};
use crate::environment;
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

#[derive(Deserialize)]
pub struct PostMessagesQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Deserialize)]
pub struct NewPostMessageDto {
    pub recipient_user_id: Option<Uuid>,
    pub title: Option<String>,
    pub content: String,
    pub attachment_type: Option<MessageAttachmentType>,
    pub attachment: Option<MessageAttachment>,
}

#[derive(Serialize)]
pub struct MessageCreationResponse {
    pub question_message: Message,
    pub pending_reply_message: Option<Message>,
}

pub(crate) fn is_service_mentor(recipient: &User, pool: &DbPool) -> Result<bool, PpdcError> {
    if recipient.principal_type == UserPrincipalType::Service {
        return Ok(true);
    }

    Ok(recipient.has_role(UserRole::Mentor, pool)?)
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
        || !recipient.allows_instant_received_message_email()
    {
        return Ok(None);
    }

    let conversation_url = if let Some(post_id) = message.post_id {
        let post = Post::find_full(post_id, pool)?;
        let source_trace = post
            .source_trace_id
            .map(|trace_id| Trace::find_full_trace(trace_id, pool))
            .transpose()?;
        let source_journal_id = source_trace.as_ref().and_then(|trace| trace.journal_id);
        let sender_is_owner = sender.id == post.user_id;

        if sender_is_owner {
            source_journal_id.map(|journal_id| {
                format!(
                    "{}/me/conversation?journal_id={}&post_id={}&view=post_chat&recipient_user_id={}",
                    environment::get_app_base_url().trim_end_matches('/'),
                    journal_id,
                    post_id,
                    sender.id
                )
            })
        } else {
            source_trace
                .and_then(|trace| {
                    trace.journal_id.map(|journal_id| {
                        format!(
                            "{}/me/conversation?journal_id={}&trace_id={}&view=trace_chat&recipient_user_id={}",
                            environment::get_app_base_url().trim_end_matches('/'),
                            journal_id,
                            trace.id,
                            sender.id
                        )
                    })
                })
                .or_else(|| {
                    source_journal_id.map(|journal_id| {
                        format!(
                            "{}/me/conversation?journal_id={}&post_id={}&view=post_chat&recipient_user_id={}",
                            environment::get_app_base_url().trim_end_matches('/'),
                            journal_id,
                            post_id,
                            sender.id
                        )
                    })
                })
        }
    } else {
        message.trace_id.as_ref().map(|trace_id| {
            Trace::find_full_trace(*trace_id, pool)
                .ok()
                .and_then(|trace| {
                    trace.journal_id.map(|journal_id| {
                        format!(
                            "{}/me/conversation?journal_id={}&trace_id={}&view=trace_chat&recipient_user_id={}",
                            environment::get_app_base_url().trim_end_matches('/'),
                            journal_id,
                            trace_id,
                            sender.id
                        )
                    })
                })
        })
        .flatten()
    };

    let template = mailer::message_received_email(
        &recipient.display_name(),
        &sender.display_name(),
        &message.content,
        conversation_url.as_deref(),
    );
    let email = NewOutboundEmail::new(
        Some(recipient.id),
        "USER_MESSAGE_RECEIVED".to_string(),
        Some("MESSAGE".to_string()),
        Some(message.id),
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
pub async fn get_post_messages_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(post_id): Path<Uuid>,
    Query(params): Query<PostMessagesQuery>,
) -> Result<Json<PaginatedResponse<Message>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let post = Post::find_full(post_id, &pool)?;
    if !PostGrant::user_can_read_post(&post, user_id, &pool)? {
        return Err(PpdcError::unauthorized());
    }

    let pagination = params.pagination.validate()?;
    let (messages, total) = Message::find_for_post_conversation_paginated(
        user_id,
        post_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(messages, pagination, total)))
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
    let sender_user = User::find(&sender_user_id, &pool)?;
    let recipient = User::find(&payload.recipient_user_id, &pool)?;
    let recipient_is_service_mentor = is_service_mentor(&recipient, &pool)?;
    let message_type = match payload.message_type {
        Some(MessageType::General) if recipient_is_service_mentor => MessageType::Question,
        Some(message_type) => message_type,
        None if recipient_is_service_mentor => MessageType::Question,
        None => MessageType::General,
    };

    if payload.post_id.is_some() && payload.trace_id.is_some() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "post_id and trace_id cannot both be provided".to_string(),
        ));
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

    if let Some(post_id) = payload.post_id {
        let post = Post::find_full(post_id, &pool)?;
        if !PostGrant::user_can_read_post(&post, sender_user_id, &pool)?
            || !PostGrant::user_can_read_post(&post, payload.recipient_user_id, &pool)?
        {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Sender and recipient must currently have access to the linked post".to_string(),
            ));
        }
    }
    if matches!(
        message_type,
        MessageType::Question | MessageType::TarotReadingRequest
    ) {
        if !sender_user.allows_ai_features() {
            return Err(PpdcError::new(
                403,
                ErrorType::ApiError,
                "AI features are disabled for this account".to_string(),
            ));
        }
        if !recipient_is_service_mentor {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "recipient_user_id must belong to a service mentor for mentor requests".to_string(),
            ));
        }

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

        let question_message = NewMessage {
            sender_user_id,
            recipient_user_id: payload.recipient_user_id,
            landscape_analysis_id: payload.landscape_analysis_id,
            trace_id: payload.trace_id,
            post_id: payload.post_id,
            reply_to_message_id: None,
            message_type,
            processing_state: MessageProcessingState::Processed,
            title: payload.title.unwrap_or_default(),
            content: payload.content,
            attachment_type: payload.attachment_type,
            attachment: payload.attachment,
            metadata: None,
        }
        .create(&pool)?;
        let pending_reply_message = NewMessage {
            sender_user_id: question_message.recipient_user_id,
            recipient_user_id: sender_user_id,
            landscape_analysis_id: question_message.landscape_analysis_id,
            trace_id: question_message.trace_id,
            post_id: None,
            reply_to_message_id: Some(question_message.id),
            message_type: MessageType::MentorReply,
            processing_state: MessageProcessingState::Pending,
            title: String::new(),
            content: String::new(),
            attachment_type: None,
            attachment: None,
            metadata: None,
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
pub async fn post_post_message_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(post_id): Path<Uuid>,
    Json(payload): Json<NewPostMessageDto>,
) -> Result<Json<MessageCreationResponse>, PpdcError> {
    let sender_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let post = Post::find_full(post_id, &pool)?;
    if !PostGrant::user_can_read_post(&post, sender_user_id, &pool)? {
        return Err(PpdcError::unauthorized());
    }

    let recipient_user_id = if sender_user_id == post.user_id {
        let recipient_user_id = payload.recipient_user_id.ok_or_else(|| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                "recipient_user_id is required when the post owner sends a message".to_string(),
            )
        })?;
        if recipient_user_id == sender_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot send a post message to yourself".to_string(),
            ));
        }
        if !PostGrant::user_can_read_post(&post, recipient_user_id, &pool)? {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Recipient must currently have access to the post".to_string(),
            ));
        }
        recipient_user_id
    } else {
        post.user_id
    };

    let message = NewMessage {
        sender_user_id,
        recipient_user_id,
        landscape_analysis_id: None,
        trace_id: None,
        post_id: Some(post_id),
        reply_to_message_id: None,
        message_type: MessageType::General,
        processing_state: MessageProcessingState::Processed,
        title: payload.title.unwrap_or_default(),
        content: payload.content,
        attachment_type: payload.attachment_type,
        attachment: payload.attachment,
        metadata: None,
    }
    .create(&pool)?;

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

    if payload.post_id.is_some() && payload.trace_id.is_some() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "post_id and trace_id cannot both be provided".to_string(),
        ));
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

    if let Some(post_id) = payload.post_id {
        let post = Post::find_full(post_id, &pool)?;
        if !PostGrant::user_can_read_post(&post, sender_user_id, &pool)?
            || !PostGrant::user_can_read_post(&post, payload.recipient_user_id, &pool)?
        {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Sender and recipient must currently have access to the linked post".to_string(),
            ));
        }
    }

    message.recipient_user_id = payload.recipient_user_id;
    message.landscape_analysis_id = payload.landscape_analysis_id;
    message.trace_id = payload.trace_id;
    message.post_id = payload.post_id;
    message.message_type = payload.message_type.unwrap_or(message.message_type);
    message.title = payload.title.unwrap_or_default();
    message.content = payload.content;
    message.attachment_type = payload.attachment_type;
    message.attachment = payload.attachment;
    let message = message.update(&pool)?;
    Ok(Json(message))
}
