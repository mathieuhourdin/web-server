use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandscapeProcessingState {
    Pending,
    Running,
    ReplayRequested,
    Completed,
    Failed,
}

impl LandscapeProcessingState {
    pub fn to_db(self) -> &'static str {
        match self {
            LandscapeProcessingState::Pending => "PENDING",
            LandscapeProcessingState::Running => "RUNNING",
            LandscapeProcessingState::ReplayRequested => "REPLAY_REQUESTED",
            LandscapeProcessingState::Completed => "COMPLETED",
            LandscapeProcessingState::Failed => "FAILED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "RUNNING" | "running" => LandscapeProcessingState::Running,
            "REPLAY_REQUESTED" | "replay_requested" => LandscapeProcessingState::ReplayRequested,
            "COMPLETED" | "completed" => LandscapeProcessingState::Completed,
            "FAILED" | "failed" => LandscapeProcessingState::Failed,
            _ => LandscapeProcessingState::Pending,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandscapeAnalysisType {
    DailyRecap,
    WeeklyRecap,
    Hlp,
    Bio,
    TraceIncremental,
}

impl LandscapeAnalysisType {
    pub fn to_db(self) -> &'static str {
        match self {
            LandscapeAnalysisType::DailyRecap => "DAILY_RECAP",
            LandscapeAnalysisType::WeeklyRecap => "WEEKLY_RECAP",
            LandscapeAnalysisType::Hlp => "HLP",
            LandscapeAnalysisType::Bio => "BIO",
            LandscapeAnalysisType::TraceIncremental => "TRACE_INCREMENTAL",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "DAILY_RECAP" | "daily_recap" => LandscapeAnalysisType::DailyRecap,
            "WEEKLY_RECAP" | "weekly_recap" => LandscapeAnalysisType::WeeklyRecap,
            "HLP" | "hlp" => LandscapeAnalysisType::Hlp,
            "BIO" | "bio" => LandscapeAnalysisType::Bio,
            _ => LandscapeAnalysisType::TraceIncremental,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandscapeAnalysisInputType {
    Primary,
    Covered,
}

impl LandscapeAnalysisInputType {
    pub fn to_db(self) -> &'static str {
        match self {
            LandscapeAnalysisInputType::Primary => "PRIMARY",
            LandscapeAnalysisInputType::Covered => "COVERED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "PRIMARY" | "primary" => LandscapeAnalysisInputType::Primary,
            _ => LandscapeAnalysisInputType::Covered,
        }
    }
}
