use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    General,
    MentorFeedback,
    Question,
    MentorReply,
    TarotReadingRequest,
}

impl MessageType {
    pub fn to_db(self) -> &'static str {
        match self {
            MessageType::General => "GENERAL",
            MessageType::MentorFeedback => "MENTOR_FEEDBACK",
            MessageType::Question => "QUESTION",
            MessageType::MentorReply => "MENTOR_REPLY",
            MessageType::TarotReadingRequest => "TAROT_READING_REQUEST",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "MENTOR_FEEDBACK" | "mentor_feedback" => MessageType::MentorFeedback,
            "QUESTION" | "question" => MessageType::Question,
            "MENTOR_REPLY" | "mentor_reply" => MessageType::MentorReply,
            "TAROT_READING_REQUEST" | "tarot_reading_request" => MessageType::TarotReadingRequest,
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
