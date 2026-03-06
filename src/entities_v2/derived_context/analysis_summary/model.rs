use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisSummaryType {
    PeriodRecap,
}

impl AnalysisSummaryType {
    pub fn to_db(self) -> &'static str {
        match self {
            AnalysisSummaryType::PeriodRecap => "PERIOD_RECAP",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "PERIOD_RECAP" | "period_recap" => AnalysisSummaryType::PeriodRecap,
            _ => AnalysisSummaryType::PeriodRecap,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisSummary {
    pub id: Uuid,
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub summary_type: AnalysisSummaryType,
    pub title: String,
    pub content: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewAnalysisSummaryDto {
    pub summary_type: Option<AnalysisSummaryType>,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct NewAnalysisSummary {
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub summary_type: AnalysisSummaryType,
    pub title: String,
    pub content: String,
}

impl NewAnalysisSummary {
    pub fn new(payload: NewAnalysisSummaryDto, landscape_analysis_id: Uuid, user_id: Uuid) -> Self {
        Self {
            landscape_analysis_id,
            user_id,
            summary_type: payload
                .summary_type
                .unwrap_or(AnalysisSummaryType::PeriodRecap),
            title: payload.title,
            content: payload.content,
        }
    }
}
