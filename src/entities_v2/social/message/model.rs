use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::attachment::{MessageAttachment, MessageAttachmentType};
pub use super::enums::{MessageProcessingState, MessageType};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MentorFeedbackScope {
    User,
    Hlp,
    Trace,
    Landmark,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MentorFeedbackMode {
    Reflection,
    Guidance,
    Playful,
    Resource,
    Technical,
    Recognition,
    Support,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MentorFeedbackTone {
    Warm,
    Light,
    Serious,
    Playful,
    Direct,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MentorFeedbackMetadata {
    pub scope: MentorFeedbackScope,
    pub feedback_mode: MentorFeedbackMode,
    pub tone: MentorFeedbackTone,
    pub subject: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MessageMetadata {
    pub mentor_feedback: Option<MentorFeedbackMetadata>,
}

impl MessageMetadata {
    pub fn mentor_feedback(metadata: MentorFeedbackMetadata) -> Self {
        Self {
            mentor_feedback: Some(metadata),
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
    pub post_id: Option<Uuid>,
    pub reply_to_message_id: Option<Uuid>,
    pub message_type: MessageType,
    pub processing_state: MessageProcessingState,
    pub title: String,
    pub content: String,
    pub attachment_type: Option<MessageAttachmentType>,
    pub attachment: Option<MessageAttachment>,
    pub metadata: Option<MessageMetadata>,
    pub seen_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewMessageDto {
    pub recipient_user_id: Uuid,
    pub landscape_analysis_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub post_id: Option<Uuid>,
    pub message_type: Option<MessageType>,
    pub title: Option<String>,
    pub content: String,
    pub attachment_type: Option<MessageAttachmentType>,
    pub attachment: Option<MessageAttachment>,
}

#[derive(Debug, Clone)]
pub struct NewMessage {
    pub sender_user_id: Uuid,
    pub recipient_user_id: Uuid,
    pub landscape_analysis_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub post_id: Option<Uuid>,
    pub reply_to_message_id: Option<Uuid>,
    pub message_type: MessageType,
    pub processing_state: MessageProcessingState,
    pub title: String,
    pub content: String,
    pub attachment_type: Option<MessageAttachmentType>,
    pub attachment: Option<MessageAttachment>,
    pub metadata: Option<MessageMetadata>,
}

impl NewMessage {
    pub fn new(payload: NewMessageDto, sender_user_id: Uuid) -> Self {
        Self {
            sender_user_id,
            recipient_user_id: payload.recipient_user_id,
            landscape_analysis_id: payload.landscape_analysis_id,
            trace_id: payload.trace_id,
            post_id: payload.post_id,
            reply_to_message_id: None,
            message_type: payload.message_type.unwrap_or(MessageType::General),
            processing_state: MessageProcessingState::Processed,
            title: payload.title.unwrap_or_default(),
            content: payload.content,
            attachment_type: payload.attachment_type,
            attachment: payload.attachment,
            metadata: None,
        }
    }
}
