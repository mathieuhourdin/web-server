use serde::{Deserialize, Serialize};
use crate::entities::{error::PpdcError};
use crate::openai_handler::gpt_responses_handler::make_gpt_request;
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::db::DbPool;
use uuid::Uuid;

pub async fn get_high_level_analysis(
    previous_context: &String,
    trace: &String,
    log_header: &str,
) -> Result<String, PpdcError> {
    let high_level_analysis = get_high_level_analysis_from_gpt(
        previous_context, 
        trace,
        log_header)
    .await?;
    Ok(high_level_analysis)

}

#[derive(Debug, Serialize, Deserialize)]
pub struct HighLevelAnalysis {
    pub title: String,
    pub subtitle: String,
    pub content: String,
}


pub async fn high_level_analysis_pipeline(previous_context: &String, landscape_analysis_id: Uuid, full_trace_string: &String, pool: &DbPool, log_header: &str) -> Result<LandscapeAnalysis, PpdcError> {
    let run_high_level_analysis = false;
    let landscape_analysis: LandscapeAnalysis = LandscapeAnalysis::find_full_analysis(landscape_analysis_id, &pool)?;

    if run_high_level_analysis {
    let new_high_level_analysis = get_high_level_analysis(
        previous_context, 
        full_trace_string,
        log_header)
        .await?;
        let mut landscape_analysis = landscape_analysis.clone();
        landscape_analysis.plain_text_state_summary = new_high_level_analysis;
        let landscape_analysis = landscape_analysis.update(&pool)?;
        return Ok(landscape_analysis);
    }
    Ok(landscape_analysis)
}

pub fn get_system_prompt() -> String {
    include_str!("elements_pipeline/prompts/landscape_analysis/system.md").to_string()
}

pub async fn get_high_level_analysis_from_gpt(
    work_context: &String,
    new_trace: &String,
    log_header: &str,
) -> Result<String, PpdcError> {
    tracing::info!(
        target: "work_analyzer",
        "{} high_level_analysis_llm_start",
        log_header
    );

    let system_prompt = get_system_prompt();

    let user_prompt = format!("
        previous_summary :
        <<<
         {}
         >>>\n\n
        new_trace :
        <<<
         {}
         >>>\n\n",
        work_context,
        new_trace
    );

    let _schema = serde_json::json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "subtitle": {"type": "string"},
            "content": {"type": "string"},
        },
        "required": ["title", "subtitle", "content"],
        "additionalProperties": false
    });

    let high_level_analysis: String =
        make_gpt_request(
            system_prompt,
            user_prompt,
            None,
            Some("Analysis / High-Level Summary"),
            Some(log_header),
            None,
        ).await?;
    Ok(high_level_analysis)
}
