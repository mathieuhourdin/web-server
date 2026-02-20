use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::error::PpdcError;
use crate::entities_v2::{landmark::Landmark, trace::Trace};
use crate::openai_handler::GptRequestConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorHeader {
    pub title: String,
    pub subtitle: String,
    pub tags: Vec<String>,
    pub high_level_projects: Vec<SelectedHighLevelProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedHighLevelProject {
    pub id: i32,
    pub span: String,
}

#[derive(Debug, Clone, Serialize)]
struct MirrorHeaderPromptInput {
    trace_text: String,
    high_level_projects: Vec<HighLevelProjectPromptItem>,
}

#[derive(Debug, Clone, Serialize)]
struct HighLevelProjectPromptItem {
    id: i32,
    title: String,
    subtitle: String,
    content: String,
}

pub fn build_high_level_project_index_map(
    high_level_projects: &[Landmark],
) -> HashMap<i32, Uuid> {
    high_level_projects
        .iter()
        .enumerate()
        .map(|(index, hlp)| (index as i32, hlp.id))
        .collect::<HashMap<_, _>>()
}

pub async fn extract_mirror_header(
    trace: &Trace,
    analysis_id: Uuid,
    high_level_projects: &[Landmark],
) -> Result<MirrorHeader, PpdcError> {
    let system_prompt = include_str!("system.md").to_string();
    let schema: serde_json::Value = serde_json::from_str(include_str!("schema.json"))?;

    let prompt_input = MirrorHeaderPromptInput {
        trace_text: trace.content.clone(),
        high_level_projects: high_level_projects
            .iter()
            .enumerate()
            .map(|(index, hlp)| HighLevelProjectPromptItem {
                id: index as i32,
                title: hlp.title.clone(),
                subtitle: hlp.subtitle.clone(),
                content: hlp.content.clone(),
            })
            .collect::<Vec<_>>(),
    };

    let user_prompt = serde_json::to_string_pretty(&prompt_input)?;
    let config = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        Some(analysis_id),
    )
    .with_display_name("Mirror / Header Extraction");

    config.execute().await
}
