use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::enums::AnalysisSummaryType;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MeaningfulEvent {
    pub title: String,
    pub description: String,
    pub event_date: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisSummary {
    pub id: Uuid,
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub summary_type: AnalysisSummaryType,
    pub title: String,
    pub short_content: String,
    pub content: String,
    pub meaningful_event: Option<MeaningfulEvent>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewAnalysisSummaryDto {
    pub summary_type: Option<AnalysisSummaryType>,
    pub title: String,
    pub short_content: Option<String>,
    pub content: String,
    pub meaningful_event: Option<MeaningfulEvent>,
}

#[derive(Debug, Clone)]
pub struct NewAnalysisSummary {
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub summary_type: AnalysisSummaryType,
    pub title: String,
    pub short_content: String,
    pub content: String,
    pub meaningful_event: Option<MeaningfulEvent>,
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
            short_content: payload.short_content.unwrap_or_else(|| payload.content.clone()),
            content: payload.content,
            meaningful_event: payload.meaningful_event,
        }
    }
}
