use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandscapeProcessingState {
    Pending,
    ReplayRequested,
    Completed,
}

impl LandscapeProcessingState {
    pub fn to_db(self) -> &'static str {
        match self {
            LandscapeProcessingState::Pending => "PENDING",
            LandscapeProcessingState::ReplayRequested => "REPLAY_REQUESTED",
            LandscapeProcessingState::Completed => "COMPLETED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "REPLAY_REQUESTED" | "replay_requested" => LandscapeProcessingState::ReplayRequested,
            "COMPLETED" | "completed" => LandscapeProcessingState::Completed,
            _ => LandscapeProcessingState::Pending,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandscapeAnalysisType {
    DailyRecap,
    TraceIncremental,
}

impl LandscapeAnalysisType {
    pub fn to_db(self) -> &'static str {
        match self {
            LandscapeAnalysisType::DailyRecap => "DAILY_RECAP",
            LandscapeAnalysisType::TraceIncremental => "TRACE_INCREMENTAL",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "DAILY_RECAP" | "daily_recap" => LandscapeAnalysisType::DailyRecap,
            _ => LandscapeAnalysisType::TraceIncremental,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LandscapeAnalysis {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub plain_text_state_summary: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub period_start: NaiveDateTime,
    pub period_end: NaiveDateTime,
    pub user_id: Uuid,
    pub parent_analysis_id: Option<Uuid>,
    pub replayed_from_id: Option<Uuid>,
    pub analyzed_trace_id: Option<Uuid>,
    pub trace_mirror_id: Option<Uuid>,
    pub landscape_analysis_type: LandscapeAnalysisType,
    pub processing_state: LandscapeProcessingState,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Struct for creating a new LandscapeAnalysis with all required data.
pub struct NewLandscapeAnalysis {
    pub title: String,
    pub subtitle: String,
    pub plain_text_state_summary: String,
    pub interaction_date: NaiveDateTime,
    pub period_start: NaiveDateTime,
    pub period_end: NaiveDateTime,
    pub user_id: Uuid,
    pub parent_analysis_id: Option<Uuid>,
    pub analyzed_trace_id: Option<Uuid>,
    pub replayed_from_id: Option<Uuid>,
    pub trace_mirror_id: Option<Uuid>,
    pub landscape_analysis_type: LandscapeAnalysisType,
}

impl NewLandscapeAnalysis {
    /// Creates a new NewLandscapeAnalysis with all fields.
    pub fn new(
        title: String,
        subtitle: String,
        plain_text_state_summary: String,
        user_id: Uuid,
        interaction_date: NaiveDateTime,
        parent_analysis_id: Option<Uuid>,
        analyzed_trace_id: Option<Uuid>,
        replayed_from_id: Option<Uuid>,
    ) -> NewLandscapeAnalysis {
        NewLandscapeAnalysis {
            title,
            subtitle,
            plain_text_state_summary,
            interaction_date,
            period_start: interaction_date,
            period_end: interaction_date,
            user_id,
            parent_analysis_id,
            analyzed_trace_id,
            replayed_from_id,
            trace_mirror_id: None,
            landscape_analysis_type: LandscapeAnalysisType::TraceIncremental,
        }
    }

    pub fn new_placeholder(
        user_id: Uuid,
        trace_id: Uuid,
        parent_landscape_analysis_id: Option<Uuid>,
        replayed_from_id: Option<Uuid>,
    ) -> NewLandscapeAnalysis {
        let now = Utc::now().naive_utc();
        NewLandscapeAnalysis {
            title: format!("Analyse de la trace {}", trace_id),
            subtitle: "Analyse".to_string(),
            plain_text_state_summary: "Analyse".to_string(),
            interaction_date: now,
            period_start: now,
            period_end: now,
            user_id,
            parent_analysis_id: parent_landscape_analysis_id,
            analyzed_trace_id: Some(trace_id),
            replayed_from_id,
            trace_mirror_id: None,
            landscape_analysis_type: LandscapeAnalysisType::TraceIncremental,
        }
    }
}
