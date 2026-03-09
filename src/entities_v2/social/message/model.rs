use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::enums::{MessageProcessingState, MessageType};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub id: Uuid,
    pub sender_user_id: Uuid,
    pub recipient_user_id: Uuid,
    pub landscape_analysis_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub reply_to_message_id: Option<Uuid>,
    pub message_type: MessageType,
    pub processing_state: MessageProcessingState,
    pub title: String,
    pub content: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewMessageDto {
    pub recipient_user_id: Uuid,
    pub landscape_analysis_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub message_type: Option<MessageType>,
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct NewMessage {
    pub sender_user_id: Uuid,
    pub recipient_user_id: Uuid,
    pub landscape_analysis_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub reply_to_message_id: Option<Uuid>,
    pub message_type: MessageType,
    pub processing_state: MessageProcessingState,
    pub title: String,
    pub content: String,
}

impl NewMessage {
    pub fn new(payload: NewMessageDto, sender_user_id: Uuid) -> Self {
        Self {
            sender_user_id,
            recipient_user_id: payload.recipient_user_id,
            landscape_analysis_id: payload.landscape_analysis_id,
            trace_id: payload.trace_id,
            reply_to_message_id: None,
            message_type: payload.message_type.unwrap_or(MessageType::General),
            processing_state: MessageProcessingState::Processed,
            title: payload.title.unwrap_or_default(),
            content: payload.content,
        }
    }
}
