use serde_json::Value;
use web_server::entities::error::PpdcError;
use web_server::openai_handler::GptRequestConfig;

#[tokio::main]
async fn main() -> Result<(), PpdcError> {
    let system_prompt = include_str!("prompts/hlp_processing/system.md").to_string();
    let trace_text = include_str!("prompts/hlp_processing/trace.md").to_string();
    let user_prompt = format!(
        "Trace type: HIGH_LEVEL_PROJECTS_DEFINITION\n\nUser trace:\n{}",
        trace_text
    );
    let schema = serde_json::from_str::<Value>(include_str!("prompts/hlp_processing/schema.json"))?;

    let request = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        None,
    )
    .with_display_name("HLP Processing / Playground");

    let result: Value = request.execute().await?;
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}
