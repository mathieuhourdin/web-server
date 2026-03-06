use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceMirrorType {
    Note,
    Journal,
    HighLevelProjects,
    Bio,
}

impl TraceMirrorType {
    pub fn to_db(self) -> &'static str {
        match self {
            TraceMirrorType::Note => "NOTE",
            TraceMirrorType::Journal => "JOURNAL",
            TraceMirrorType::HighLevelProjects => "HIGH_LEVEL_PROJECTS",
            TraceMirrorType::Bio => "BIO",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "JOURNAL" | "journal" => TraceMirrorType::Journal,
            "HIGH_LEVEL_PROJECTS" | "high_level_projects" => TraceMirrorType::HighLevelProjects,
            "BIO" | "bio" => TraceMirrorType::Bio,
            _ => TraceMirrorType::Note,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TraceMirror {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub trace_mirror_type: TraceMirrorType,
    pub tags: Vec<String>,
    pub trace_id: Uuid,
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub primary_resource_id: Option<Uuid>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Struct for creating a new TraceMirror with all required data.
pub struct NewTraceMirror {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub trace_mirror_type: TraceMirrorType,
    pub tags: Vec<String>,
    pub trace_id: Uuid,
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub primary_resource_id: Option<Uuid>,
    pub interaction_date: Option<NaiveDateTime>,
}

impl NewTraceMirror {
    /// Creates a new NewTraceMirror with all fields.
    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        trace_mirror_type: TraceMirrorType,
        tags: Vec<String>,
        trace_id: Uuid,
        landscape_analysis_id: Uuid,
        user_id: Uuid,
        primary_resource_id: Option<Uuid>,
        interaction_date: Option<NaiveDateTime>,
    ) -> NewTraceMirror {
        NewTraceMirror {
            title,
            subtitle,
            content,
            trace_mirror_type,
            tags,
            trace_id,
            landscape_analysis_id,
            user_id,
            primary_resource_id,
            interaction_date,
        }
    }
}
