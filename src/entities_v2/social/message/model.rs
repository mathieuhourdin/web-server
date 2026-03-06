use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    General,
    MentorFeedback,
    Question,
    MentorReply,
}

impl MessageType {
    pub fn to_db(self) -> &'static str {
        match self {
            MessageType::General => "GENERAL",
            MessageType::MentorFeedback => "MENTOR_FEEDBACK",
            MessageType::Question => "QUESTION",
            MessageType::MentorReply => "MENTOR_REPLY",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "MENTOR_FEEDBACK" | "mentor_feedback" => MessageType::MentorFeedback,
            "QUESTION" | "question" => MessageType::Question,
            "MENTOR_REPLY" | "mentor_reply" => MessageType::MentorReply,
            _ => MessageType::General,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageProcessingState {
    Pending,
    Running,
    Processed,
    Failed,
}

impl MessageProcessingState {
    pub fn to_db(self) -> &'static str {
        match self {
            MessageProcessingState::Pending => "PENDING",
            MessageProcessingState::Running => "RUNNING",
            MessageProcessingState::Processed => "PROCESSED",
            MessageProcessingState::Failed => "FAILED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "PENDING" | "pending" => MessageProcessingState::Pending,
            "RUNNING" | "running" => MessageProcessingState::Running,
            "FAILED" | "failed" => MessageProcessingState::Failed,
            _ => MessageProcessingState::Processed,
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
