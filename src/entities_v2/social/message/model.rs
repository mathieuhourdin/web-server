use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    General,
    MentorFeedback,
}

impl MessageType {
    pub fn to_db(self) -> &'static str {
        match self {
            MessageType::General => "GENERAL",
            MessageType::MentorFeedback => "MENTOR_FEEDBACK",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "MENTOR_FEEDBACK" | "mentor_feedback" => MessageType::MentorFeedback,
            _ => MessageType::General,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub id: Uuid,
    pub sender_user_id: Uuid,
    pub recipient_user_id: Uuid,
    pub landscape_analysis_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub message_type: MessageType,
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
    pub message_type: MessageType,
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
            message_type: payload.message_type.unwrap_or(MessageType::General),
            title: payload.title.unwrap_or_default(),
            content: payload.content,
        }
    }
}
