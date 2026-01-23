use serde::{Deserialize, Serialize};
use crate::entities::{resource::Resource, error::PpdcError};
use crate::openai_handler::gpt_responses_handler::make_gpt_request;
use crate::entities_v2::{landscape_analysis::LandscapeAnalysis, trace::Trace};
use crate::work_analyzer::context_builder::create_work_context;

pub async fn get_high_level_analysis(
    previous_context: &String,
    trace: &String
) -> Result<String, PpdcError> {
    let high_level_analysis = get_high_level_analysis_from_gpt(
        previous_context, 
        trace)
    .await?;
    Ok(high_level_analysis)

}

#[derive(Debug, Serialize, Deserialize)]
pub struct HighLevelAnalysis {
    pub title: String,
    pub subtitle: String,
    pub content: String,
}

pub fn get_system_prompt() -> String {
    include_str!("trace_broker/prompts/landscape_analysis/system.md").to_string()
}

pub async fn get_high_level_analysis_from_gpt(
    work_context: &String,
    new_trace: &String
) -> Result<String, PpdcError> {
    println!("work_analyzer::high_level_analysis::get_high_level_analysis_from_gpt: Getting high level analysis from gpt string");

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

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "subtitle": {"type": "string"},
            "content": {"type": "string"},
        },
        "required": ["title", "subtitle", "content"],
        "additionalProperties": false
    });

    let high_level_analysis: String = make_gpt_request(system_prompt, user_prompt, None).await?;
    Ok(high_level_analysis)
}