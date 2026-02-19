use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::error::PpdcError;
use crate::openai_handler::GptRequestConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighLevelProjectDraft {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub spans: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HlpExtractionResult {
    pub projects: Vec<HighLevelProjectDraft>,
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
    let system_prompt = include_str!("prompts/hlp_processing/system.md").to_string();
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("prompts/hlp_processing/schema.json"))?;
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
