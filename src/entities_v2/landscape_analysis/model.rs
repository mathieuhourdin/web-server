use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::resource::maturing_state::MaturingState;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LandscapeAnalysis {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub plain_text_state_summary: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub user_id: Uuid,
    pub parent_analysis_id: Option<Uuid>,
    pub replayed_from_id: Option<Uuid>,
    pub analyzed_trace_id: Option<Uuid>,
    pub trace_mirror_id: Option<Uuid>,
    pub processing_state: MaturingState,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Struct for creating a new LandscapeAnalysis with all required data.
pub struct NewLandscapeAnalysis {
    pub title: String,
    pub subtitle: String,
    pub plain_text_state_summary: String,
    pub interaction_date: NaiveDateTime,
    pub user_id: Uuid,
    pub parent_analysis_id: Option<Uuid>,
    pub analyzed_trace_id: Option<Uuid>,
    pub replayed_from_id: Option<Uuid>,
    pub trace_mirror_id: Option<Uuid>,
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
            user_id,
            parent_analysis_id,
            analyzed_trace_id,
            replayed_from_id,
            trace_mirror_id: None,
        }
    }

    pub fn new_placeholder(
        user_id: Uuid,
        trace_id: Uuid,
        parent_landscape_analysis_id: Option<Uuid>,
        replayed_from_id: Option<Uuid>,
    ) -> NewLandscapeAnalysis {
        NewLandscapeAnalysis {
            title: format!("Analyse de la trace {}", trace_id),
            subtitle: "Analyse".to_string(),
            plain_text_state_summary: "Analyse".to_string(),
            interaction_date: Utc::now().naive_utc(),
            user_id,
            parent_analysis_id: parent_landscape_analysis_id,
            analyzed_trace_id: Some(trace_id),
            replayed_from_id,
            trace_mirror_id: None,
        }
    }
}
