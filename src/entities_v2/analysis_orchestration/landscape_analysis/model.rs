use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::enums::{
    LandscapeAnalysisInputType, LandscapeAnalysisType, LandscapeProcessingState,
};

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LandscapeAnalysisInput {
    pub id: Uuid,
    pub landscape_analysis_id: Uuid,
    pub trace_id: Option<Uuid>,
    pub trace_mirror_id: Option<Uuid>,
    pub input_type: LandscapeAnalysisInputType,
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
    pub fn new_trace_incremental(
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

    pub fn new_daily_recap(
        title: String,
        subtitle: String,
        plain_text_state_summary: String,
        user_id: Uuid,
        interaction_date: NaiveDateTime,
        period_start: NaiveDateTime,
        period_end: NaiveDateTime,
        parent_analysis_id: Option<Uuid>,
        replayed_from_id: Option<Uuid>,
    ) -> NewLandscapeAnalysis {
        NewLandscapeAnalysis {
            title,
            subtitle,
            plain_text_state_summary,
            interaction_date,
            period_start,
            period_end,
            user_id,
            parent_analysis_id,
            analyzed_trace_id: None,
            replayed_from_id,
            trace_mirror_id: None,
            landscape_analysis_type: LandscapeAnalysisType::DailyRecap,
        }
    }

    pub fn new_weekly_recap(
        title: String,
        subtitle: String,
        plain_text_state_summary: String,
        user_id: Uuid,
        interaction_date: NaiveDateTime,
        period_start: NaiveDateTime,
        period_end: NaiveDateTime,
        parent_analysis_id: Option<Uuid>,
        replayed_from_id: Option<Uuid>,
    ) -> NewLandscapeAnalysis {
        NewLandscapeAnalysis {
            title,
            subtitle,
            plain_text_state_summary,
            interaction_date,
            period_start,
            period_end,
            user_id,
            parent_analysis_id,
            analyzed_trace_id: None,
            replayed_from_id,
            trace_mirror_id: None,
            landscape_analysis_type: LandscapeAnalysisType::WeeklyRecap,
        }
    }

    pub fn new_hlp(
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
            landscape_analysis_type: LandscapeAnalysisType::Hlp,
        }
    }

    pub fn new_bio(
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
            landscape_analysis_type: LandscapeAnalysisType::Bio,
        }
    }

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
        NewLandscapeAnalysis::new_trace_incremental(
            title,
            subtitle,
            plain_text_state_summary,
            user_id,
            interaction_date,
            parent_analysis_id,
            analyzed_trace_id,
            replayed_from_id,
        )
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
