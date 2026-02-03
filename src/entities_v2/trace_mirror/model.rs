use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TraceMirror {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub tags: Vec<String>,
    pub trace_id: Uuid,
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub primary_resource_id: Option<Uuid>,
    pub primary_theme_id: Option<Uuid>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Struct for creating a new TraceMirror with all required data.
pub struct NewTraceMirror {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub tags: Vec<String>,
    pub trace_id: Uuid,
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub primary_resource_id: Option<Uuid>,
    pub primary_theme_id: Option<Uuid>,
    pub interaction_date: Option<NaiveDateTime>,
}

impl NewTraceMirror {
    /// Creates a new NewTraceMirror with all fields.
    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        tags: Vec<String>,
        trace_id: Uuid,
        landscape_analysis_id: Uuid,
        user_id: Uuid,
        primary_resource_id: Option<Uuid>,
        primary_theme_id: Option<Uuid>,
        interaction_date: Option<NaiveDateTime>,
    ) -> NewTraceMirror {
        NewTraceMirror {
            title,
            subtitle,
            content,
            tags,
            trace_id,
            landscape_analysis_id,
            user_id,
            primary_resource_id,
            primary_theme_id,
            interaction_date,
        }
    }
}
