use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisEvent {
    pub analysis_id: Uuid,
    pub seq: u64,
    pub timestamp: NaiveDateTime,
    pub event: AnalysisEventData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AnalysisEventData {
    StepStarted {
        step: String,
    },
    StepFinished {
        step: String,
        summary: Option<String>,
    },

    LlmCallStarted {
        step: String,
        llm_call_id: String,
    },
    LlmCallFinished {
        step: String,
        llm_call_id: String,
        ok: bool,
    },

    CandidatesRetrieved {
        step: String,
        local_id: String,
        landmark_type: String,
        k: usize,
        candidates: Vec<CandidateBrief>,
    },

    DecisionMade {
        step: String,
        local_id: String,
        decision: Decision,
        confidence: f32,
        rationale: Option<String>,
    },

    Error {
        step: String,
        message: String,
        retry_in_seconds: Option<u64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateBrief {
    pub landmark_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Decision {
    Match { landmark_id: String },
    Create { proposed_title: String },
    Skip,
}
