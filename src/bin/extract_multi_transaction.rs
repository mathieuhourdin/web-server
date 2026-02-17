use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;

use chrono::Utc;
use web_server::entities::error::PpdcError;
use web_server::work_analyzer::elements_pipeline::extraction_multi_transaction::extract_multi_transaction_from_test_trace;

#[tokio::main]
async fn main() -> Result<(), PpdcError> {
    let extraction = extract_multi_transaction_from_test_trace().await?;
    let output_json = serde_json::to_string_pretty(&extraction)?;
    println!("{}", output_json);

    let system_prompt = include_str!(
        "../work_analyzer/elements_pipeline/prompts/landmark_resource/extraction/multi_transaction_system.md"
    );
    let trace_text = include_str!(
        "../work_analyzer/elements_pipeline/prompts/landmark_resource/extraction/multi_transaction_test_trace.md"
    );
    let user_prompt = format!("trace_text :\n{}", trace_text);
    let schema = serde_json::from_str::<serde_json::Value>(include_str!(
        "../work_analyzer/elements_pipeline/prompts/landmark_resource/extraction/multi_transaction_schema.json"
    ))?;
    let schema_json = serde_json::to_string_pretty(&schema)?;

    let mut log_path = "logs/extract_multi_transaction.log";
    let mut log_file = if create_dir_all("logs").is_ok() {
        match OpenOptions::new().create(true).append(true).open(log_path) {
            Ok(file) => file,
            Err(_) => {
                log_path = "extract_multi_transaction.log";
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
        log_path = "extract_multi_transaction.log";
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| PpdcError::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))?
    };

    writeln!(
        log_file,
        "===== extract_multi_transaction {} =====\nSYSTEM PROMPT:\n{}\n\nUSER PROMPT:\n{}\n\nSCHEMA:\n{}\n\nGPT OUTPUT:\n{}\n",
        Utc::now().to_rfc3339(),
        system_prompt,
        user_prompt,
        schema_json,
        output_json
    )
    .map_err(|e| PpdcError::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))?;
    println!("Logged extraction to {}", log_path);

    Ok(())
}
