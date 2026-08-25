use serde::Deserialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::message::{
    Message, MessageAttachment, MessageAttachmentType, MessageProcessingState, MessageType,
};
use crate::entities_v2::post::{Post, PostStatus};
use crate::entities_v2::post_grant::PostGrant;
use crate::entities_v2::trace::Trace;
use crate::entities_v2::user::User;
use crate::openai_handler::{GptReasoningEffort, GptRequestConfig, GptVerbosity};
use crate::work_analyzer::MENTOR_OPENAI_MODEL;

use super::context::{build as build_context, build_shared_trace};

#[derive(Debug, Deserialize)]
struct TraceReplyDraft {
    title: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct TarotReplyDraft {
    title: String,
    content: String,
}

pub async fn run_message(message_id: Uuid, pool: &DbPool) -> Result<Message, PpdcError> {
    let mut reply_message = Message::find(message_id, pool)?;
    reply_message.processing_state = MessageProcessingState::Running;
    reply_message = reply_message.update(pool)?;

    let result = run_message_inner(reply_message.clone(), pool).await;
    match result {
        Ok(message) => Ok(message),
        Err(err) => {
            if let Ok(mut failed_message) = Message::find(message_id, pool) {
                failed_message.processing_state = MessageProcessingState::Failed;
                let _ = failed_message.update(pool);
            }
            Err(err)
        }
    }
}

async fn run_message_inner(reply_message: Message, pool: &DbPool) -> Result<Message, PpdcError> {
    if reply_message.message_type != MessageType::MentorReply {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Message {} is not a mentor reply", reply_message.id),
        ));
    }

    let question_message_id = reply_message.reply_to_message_id.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            format!(
                "Reply message {} is missing reply_to_message_id",
                reply_message.id
            ),
        )
    })?;
    let question_message = Message::find(question_message_id, pool)?;
    let trace = reply_message
        .trace_id
        .or(question_message.trace_id)
        .map(|trace_id| Trace::find_full_trace(trace_id, pool))
        .transpose()?;
    let mentor_user = User::find(&reply_message.sender_user_id, pool)?;
    let recipient_user = User::find(&reply_message.recipient_user_id, pool)?;
    if !recipient_user.allows_ai_features() {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            format!(
                "AI features are disabled for recipient user {}",
                recipient_user.id
            ),
        ));
    }

    if matches!(
        question_message.message_type,
        MessageType::SharedTraceExplanationRequest | MessageType::SharedTraceTranslationRequest
    ) {
        let trace = trace.as_ref().ok_or_else(|| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                "Shared-trace mentor requests require a trace".to_string(),
            )
        })?;
        ensure_reader_can_still_access_shared_trace(trace.id, recipient_user.id, pool)?;
    }

    match question_message.message_type {
        MessageType::TarotReadingRequest => {
            run_tarot_reply_pipeline(
                &reply_message,
                &question_message,
                trace.as_ref(),
                &mentor_user,
                &recipient_user,
                pool,
            )
            .await
        }
        MessageType::SharedTraceExplanationRequest => {
            run_shared_trace_reply_pipeline(
                &reply_message,
                &question_message,
                trace.as_ref().ok_or_else(|| {
                    PpdcError::new(
                        400,
                        ErrorType::ApiError,
                        "Shared-trace explanation requests require a trace".to_string(),
                    )
                })?,
                &mentor_user,
                &recipient_user,
                include_str!("shared_trace_explanation_system.md"),
                "Message Processing / Shared Trace Explanation",
                pool,
            )
            .await
        }
        MessageType::SharedTraceTranslationRequest => {
            let has_translation_attachment = matches!(
                (
                    question_message.attachment_type,
                    question_message.attachment.as_ref()
                ),
                (
                    Some(MessageAttachmentType::SharedTraceTranslation),
                    Some(MessageAttachment::SharedTraceTranslation(attachment))
                ) if !attachment.target_locale.trim().is_empty()
            );
            if !has_translation_attachment {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Shared-trace translation request is missing its target locale".to_string(),
                ));
            }
            run_shared_trace_reply_pipeline(
                &reply_message,
                &question_message,
                trace.as_ref().ok_or_else(|| {
                    PpdcError::new(
                        400,
                        ErrorType::ApiError,
                        "Shared-trace translation requests require a trace".to_string(),
                    )
                })?,
                &mentor_user,
                &recipient_user,
                include_str!("shared_trace_translation_system.md"),
                "Message Processing / Shared Trace Translation",
                pool,
            )
            .await
        }
        _ => {
            run_standard_reply_pipeline(
                &reply_message,
                &question_message,
                trace.as_ref(),
                &mentor_user,
                &recipient_user,
                pool,
            )
            .await
        }
    }
}

fn ensure_reader_can_still_access_shared_trace(
    trace_id: Uuid,
    reader_user_id: Uuid,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let Some(post) = Post::find_for_trace(trace_id, pool)? else {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "The shared trace is no longer published".to_string(),
        ));
    };
    if post.status != PostStatus::Published
        || !PostGrant::user_can_read_post(&post, reader_user_id, pool)?
    {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "The reader no longer has access to this shared trace".to_string(),
        ));
    }
    Ok(())
}

async fn run_shared_trace_reply_pipeline(
    reply_message: &Message,
    question_message: &Message,
    shared_trace: &Trace,
    mentor_user: &User,
    reader_user: &User,
    system_prompt: &str,
    display_name: &str,
    pool: &DbPool,
) -> Result<Message, PpdcError> {
    let prompt_context = build_shared_trace(
        reply_message,
        question_message,
        shared_trace,
        mentor_user,
        reader_user,
        pool,
    )?;
    let schema: serde_json::Value = serde_json::from_str(include_str!("schema.json"))?;
    let user_prompt = serde_json::to_string_pretty(&prompt_context)?;
    let reply = GptRequestConfig::new(
        MENTOR_OPENAI_MODEL.to_string(),
        system_prompt.to_string(),
        user_prompt,
        Some(schema),
        None,
    )
    .with_reasoning_effort(GptReasoningEffort::Low)
    .with_verbosity(GptVerbosity::Low)
    .with_display_name(display_name)
    .execute::<TraceReplyDraft>()
    .await?;

    let mut processed_message = Message::find(reply_message.id, pool)?;
    processed_message.title = reply.title;
    processed_message.content = reply.content;
    processed_message.attachment_type = None;
    processed_message.attachment = None;
    processed_message.processing_state = MessageProcessingState::Processed;
    processed_message.update(pool)
}

async fn run_standard_reply_pipeline(
    reply_message: &Message,
    question_message: &Message,
    trace: Option<&Trace>,
    mentor_user: &User,
    recipient_user: &User,
    pool: &DbPool,
) -> Result<Message, PpdcError> {
    let prompt_context = build_context(
        reply_message,
        question_message,
        trace,
        mentor_user,
        recipient_user,
        pool,
    )?;

    let system_prompt = include_str!("system.md").to_string();
    let schema: serde_json::Value = serde_json::from_str(include_str!("schema.json"))?;
    let user_prompt = serde_json::to_string_pretty(&prompt_context)?;
    let reply = GptRequestConfig::new(
        MENTOR_OPENAI_MODEL.to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        None,
    )
    .with_reasoning_effort(GptReasoningEffort::Low)
    .with_verbosity(GptVerbosity::Low)
    .with_display_name("Message Processing / Mentor Reply")
    .execute::<TraceReplyDraft>()
    .await?;

    let mut processed_message = Message::find(reply_message.id, pool)?;
    processed_message.title = reply.title;
    processed_message.content = reply.content;
    processed_message.attachment_type = None;
    processed_message.attachment = None;
    processed_message.processing_state = MessageProcessingState::Processed;
    processed_message.update(pool)
}

async fn run_tarot_reply_pipeline(
    reply_message: &Message,
    question_message: &Message,
    trace: Option<&Trace>,
    mentor_user: &User,
    recipient_user: &User,
    pool: &DbPool,
) -> Result<Message, PpdcError> {
    let has_tarot_attachment = matches!(
        (
            question_message.attachment_type,
            question_message.attachment.as_ref()
        ),
        (
            Some(MessageAttachmentType::TarotReading),
            Some(MessageAttachment::TarotReading(_))
        )
    );
    if !has_tarot_attachment {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            format!(
                "Tarot reading request message {} must include a tarot reading attachment",
                question_message.id
            ),
        ));
    }

    let prompt_context = build_context(
        reply_message,
        question_message,
        trace,
        mentor_user,
        recipient_user,
        pool,
    )?;

    let system_prompt = include_str!("tarot_system.md").to_string();
    let schema: serde_json::Value = serde_json::from_str(include_str!("tarot_schema.json"))?;
    let user_prompt = serde_json::to_string_pretty(&prompt_context)?;
    let reply = GptRequestConfig::new(
        MENTOR_OPENAI_MODEL.to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        None,
    )
    .with_reasoning_effort(GptReasoningEffort::Low)
    .with_verbosity(GptVerbosity::Low)
    .with_display_name("Message Processing / Tarot Reading Reply")
    .execute::<TarotReplyDraft>()
    .await?;

    let mut processed_message = Message::find(reply_message.id, pool)?;
    processed_message.title = reply.title;
    processed_message.content = reply.content;
    processed_message.attachment_type = None;
    processed_message.attachment = None;
    processed_message.processing_state = MessageProcessingState::Processed;
    processed_message.update(pool)
}
