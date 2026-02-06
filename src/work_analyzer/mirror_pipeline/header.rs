use crate::entities_v2::trace::Trace;
use crate::entities::error::PpdcError;
use crate::openai_handler::GptRequestConfig;
use serde::Deserialize;
use uuid::Uuid;
use crate::entities_v2::trace_mirror::model::NewTraceMirror;
use crate::entities_v2::trace_mirror::TraceMirror;
use crate::work_analyzer::analysis_processor::AnalysisContext;

#[derive(Debug, Clone, Deserialize)]
pub struct MirrorHeader {
    pub title: String,
    pub subtitle: String,
    pub tags: Vec<String>,
}

/// Extracts a MirrorHeader (title, subtitle, tags) from the trace content using the LLM.
pub async fn extract_mirror_header(trace: &Trace, analysis_id: Uuid) -> Result<MirrorHeader, PpdcError> {
    let log_header = format!("analysis_id: {}", analysis_id);
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
        Some(analysis_id),
    ).with_log_header(log_header);
    config.execute().await
}


pub async fn create_trace_mirror(trace: &Trace, header: MirrorHeader, context: &AnalysisContext) -> Result<TraceMirror, PpdcError> {
    let trace_mirror = NewTraceMirror::new(
        header.title,
        header.subtitle,
        trace.content.clone(),
        header.tags,
        trace.id,
        context.analysis_id,
        context.user_id,
        None, // primary_resource_id
        None, // primary_theme_id
        None, // interaction_date
    );
    let trace_mirror = trace_mirror.create(&context.pool)?;
    Ok(trace_mirror)
}
