use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    General,
    MentorFeedback,
    Question,
    MentorReply,
    TarotReadingRequest,
    SharedTraceExplanationRequest,
    SharedTraceTranslationRequest,
    JournalFeedbackRequest,
}

impl MessageType {
    pub fn to_db(self) -> &'static str {
        match self {
            MessageType::General => "GENERAL",
            MessageType::MentorFeedback => "MENTOR_FEEDBACK",
            MessageType::Question => "QUESTION",
            MessageType::MentorReply => "MENTOR_REPLY",
            MessageType::TarotReadingRequest => "TAROT_READING_REQUEST",
            MessageType::SharedTraceExplanationRequest => "SHARED_TRACE_EXPLANATION_REQUEST",
            MessageType::SharedTraceTranslationRequest => "SHARED_TRACE_TRANSLATION_REQUEST",
            MessageType::JournalFeedbackRequest => "JOURNAL_FEEDBACK_REQUEST",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "MENTOR_FEEDBACK" | "mentor_feedback" => MessageType::MentorFeedback,
            "QUESTION" | "question" => MessageType::Question,
            "MENTOR_REPLY" | "mentor_reply" => MessageType::MentorReply,
            "TAROT_READING_REQUEST" | "tarot_reading_request" => MessageType::TarotReadingRequest,
            "SHARED_TRACE_EXPLANATION_REQUEST" | "shared_trace_explanation_request" => {
                MessageType::SharedTraceExplanationRequest
            }
            "SHARED_TRACE_TRANSLATION_REQUEST" | "shared_trace_translation_request" => {
                MessageType::SharedTraceTranslationRequest
            }
            "JOURNAL_FEEDBACK_REQUEST" | "journal_feedback_request" => {
                MessageType::JournalFeedbackRequest
            }
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

#[cfg(test)]
mod tests {
    use super::MessageType;

    #[test]
    fn journal_feedback_request_has_stable_api_and_database_names() {
        let json = serde_json::to_string(&MessageType::JournalFeedbackRequest).unwrap();
        assert_eq!(json, "\"journal_feedback_request\"");
        assert_eq!(
            MessageType::JournalFeedbackRequest.to_db(),
            "JOURNAL_FEEDBACK_REQUEST"
        );
        assert_eq!(
            MessageType::from_db("JOURNAL_FEEDBACK_REQUEST"),
            MessageType::JournalFeedbackRequest
        );
    }
}
