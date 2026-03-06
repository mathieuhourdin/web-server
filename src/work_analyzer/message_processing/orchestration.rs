use serde::Deserialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::message::{Message, MessageProcessingState, MessageType};
use crate::entities_v2::trace::Trace;
use crate::entities_v2::user::User;
use crate::openai_handler::GptRequestConfig;

use super::context::build as build_context;

#[derive(Debug, Deserialize)]
struct TraceReplyDraft {
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
            format!("Reply message {} is missing reply_to_message_id", reply_message.id),
        )
    })?;
    let question_message = Message::find(question_message_id, pool)?;
    let trace_id = reply_message.trace_id.or(question_message.trace_id).ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Message {} is not linked to a trace", reply_message.id),
        )
    })?;

    let trace = Trace::find_full_trace(trace_id, pool)?;
    let mentor_user = User::find(&reply_message.sender_user_id, pool)?;
    let recipient_user = User::find(&reply_message.recipient_user_id, pool)?;
    let prompt_context = build_context(
        &reply_message,
        &question_message,
        &trace,
        &mentor_user,
        &recipient_user,
        pool,
    )?;

    let system_prompt = include_str!("system.md").to_string();
    let schema: serde_json::Value = serde_json::from_str(include_str!("schema.json"))?;
    let user_prompt = serde_json::to_string_pretty(&prompt_context)?;
    let reply = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        None,
    )
    .with_display_name("Message Processing / Mentor Reply")
    .execute::<TraceReplyDraft>()
    .await?;

    let mut processed_message = Message::find(reply_message.id, pool)?;
    processed_message.title = reply.title;
    processed_message.content = reply.content;
    processed_message.processing_state = MessageProcessingState::Processed;
    processed_message.update(pool)
}
