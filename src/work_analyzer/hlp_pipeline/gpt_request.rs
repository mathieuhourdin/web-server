use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::error::PpdcError;
use crate::openai_handler::GptRequestConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighLevelProjectDraft {
    pub id: i32,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub spans: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedLandmarkDraft {
    pub id: i32,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub landmark_type: crate::entities_v2::landmark::LandmarkType,
    pub spans: Vec<String>,
    pub related_project_ids: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HlpExtractionResult {
    pub projects: Vec<HighLevelProjectDraft>,
    pub landmarks: Vec<RelatedLandmarkDraft>,
}

#[derive(Debug, Clone, Serialize)]
struct HlpPromptInput {
    trace_type: &'static str,
    trace_text: String,
}

pub async fn extract_high_level_projects(
    trace_text: &str,
    analysis_id: Uuid,
) -> Result<HlpExtractionResult, PpdcError> {
    let system_prompt = include_str!("system.md").to_string();
    let schema: serde_json::Value = serde_json::from_str(include_str!("schema.json"))?;
    let user_prompt = serde_json::to_string_pretty(&HlpPromptInput {
        trace_type: "HIGH_LEVEL_PROJECTS_DEFINITION",
        trace_text: trace_text.to_string(),
    })?;

    let request = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        Some(analysis_id),
    )
    .with_display_name("HLP Pipeline / Extraction");

    request.execute().await
}
