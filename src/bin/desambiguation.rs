use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;

use chrono::Utc;
use web_server::entities::error::PpdcError;
use web_server::openai_handler::GptRequestConfig;

#[tokio::main]
async fn main() -> Result<(), PpdcError> {
    let system_prompt = include_str!("prompts/desambiguation/prompt.md").to_string();
    let trace_text = include_str!("prompts/test_traces/trace_6.md");
    let landmarks = include_str!("prompts/desambiguation/landmarks.json");
    let user_prompt = format!("User text:\n{}\n\nLandmarks:\n{}", trace_text, landmarks);
    let schema = serde_json::from_str::<serde_json::Value>(include_str!(
        "prompts/desambiguation/schema.json"
    ))?;

    let request = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        None,
    )
    .with_display_name("Desambiguation / Playground");

    let result: serde_json::Value = request.execute().await?;
    let output_json = serde_json::to_string_pretty(&result)?;
    println!("{}", output_json);

    let mut log_path = "logs/desambiguation.log";
    let mut log_file = if create_dir_all("logs").is_ok() {
        match OpenOptions::new().create(true).append(true).open(log_path) {
            Ok(file) => file,
            Err(_) => {
                log_path = "desambiguation.log";
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                    .map_err(|e| {
                        PpdcError::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                    })?
            }
        }
    } else {
        log_path = "desambiguation.log";
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| PpdcError::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))?
    };

    writeln!(
        log_file,
        "===== desambiguation {} =====\nSYSTEM PROMPT:\n{}\n\nUSER PROMPT:\n{}\n\nOUTPUT:\n{}\n",
        Utc::now().to_rfc3339(),
        request.system_prompt,
        request.user_prompt,
        output_json
    )
    .map_err(|e| PpdcError::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))?;
    println!("Logged extraction to {}", log_path);

    Ok(())
}
