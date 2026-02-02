use crate::entities_v2::trace::Trace;
use crate::entities::error::PpdcError;
use crate::openai_handler::GptRequestConfig;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MirrorHeader {
    pub title: String,
    pub subtitle: String,
    pub tags: Vec<String>,
}

/// Extracts a MirrorHeader (title, subtitle, tags) from the trace content using the LLM.
pub async fn extract_mirror_header(trace: &Trace) -> Result<MirrorHeader, PpdcError> {
    let system_prompt = include_str!("prompts/mirror_header/system.md").to_string();
    let schema_str = include_str!("prompts/mirror_header/schema.json");
    let schema: serde_json::Value = serde_json::from_str(schema_str)?;
    let user_prompt = format!(
        "trace_text :\n{}\n",
        trace.content
    );
    let config = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
    );
    config.execute().await
}