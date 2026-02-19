use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use web_server::entities::error::PpdcError;
use web_server::openai_handler::GptRequestConfig;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LandmarkSummary {
    pub title: String,
    pub content: String,
    pub landmark_type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReferenceItem {
    pub tag_id: i32,
    pub mention: String,
    pub landmark: LandmarkSummary,
}

#[derive(Deserialize, Debug, Clone)]
struct FullContextInput {
    #[serde(alias = "trace_text")]
    pub tagged_text: String,
    pub references: Vec<ReferenceItem>,
}

fn compact_text(text: &str, max_len: usize) -> String {
    let single_line = text.replace('\n', " ");
    let mut chars = single_line.chars();
    let truncated: String = chars.by_ref().take(max_len).collect();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

fn extract_spans(item: &serde_json::Value) -> Vec<String> {
    let mut spans = Vec::new();

    if let Some(array) = item.get("spans").and_then(|value| value.as_array()) {
        for value in array {
            if let Some(span) = value.as_str() {
                spans.push(span.to_string());
            }
        }
    } else if let Some(span) = item.get("spans").and_then(|value| value.as_str()) {
        spans.push(span.to_string());
    }

    // Backward compatibility with older prompt versions.
    if spans.is_empty() {
        if let Some(array) = item.get("span").and_then(|value| value.as_array()) {
            for value in array {
                if let Some(span) = value.as_str() {
                    spans.push(span.to_string());
                }
            }
        } else if let Some(span) = item.get("span").and_then(|value| value.as_str()) {
            spans.push(span.to_string());
        }
    }

    spans
}

fn collect_non_matching_spans(result: &serde_json::Value, trace_text: &str) -> Vec<String> {
    let mut mismatches = Vec::new();
    for section in ["transactions", "descriptives", "normatives", "evaluatives"] {
        let Some(items) = result.get(section).and_then(|value| value.as_array()) else {
            continue;
        };

        for (index, item) in items.iter().enumerate() {
            let claim_id = item
                .get("id")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("#{}", index));
            for (span_index, span) in extract_spans(item).iter().enumerate() {
                if !trace_text.contains(span) {
                    mismatches.push(format!(
                        "{}:{}[{}] `{}`",
                        section,
                        claim_id,
                        span_index,
                        compact_text(span, 80)
                    ));
                }
            }
        }
    }
    mismatches
}

#[tokio::main]
async fn main() -> Result<(), PpdcError> {
    let system_prompt = include_str!("prompts/gramatical_extraction/v2/system.md").to_string();
    let full_context: FullContextInput = serde_json::from_str(include_str!(
        "prompts/gramatical_extraction/full_context.json"
    ))?;
    let trace_text = full_context.tagged_text.clone();
    let user_prompt_payload = serde_json::json!({
        "trace_text": trace_text,
        "references": full_context.references,
    });
    let user_prompt = serde_json::to_string_pretty(&user_prompt_payload)?;
    let schema = serde_json::from_str::<serde_json::Value>(include_str!(
        "prompts/gramatical_extraction/v2/schema.json"
    ))?;

    let request = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        None,
    )
    .with_display_name("Gramatical extraction / Playground");

    let result: serde_json::Value = request.execute().await?;
    let output_json = serde_json::to_string_pretty(&result)?;
    println!("{}", output_json);

    let non_matching_spans = collect_non_matching_spans(&result, &trace_text);
    if !non_matching_spans.is_empty() {
        println!(
            "Span check: {} non-matching span(s) found",
            non_matching_spans.len()
        );
        for mismatch in non_matching_spans.iter().take(12) {
            println!("- {}", mismatch);
        }
        if non_matching_spans.len() > 12 {
            println!("... and {} more", non_matching_spans.len() - 12);
        }
    }

    let mut log_path = "logs/gramatical_extraction.log";
    let mut log_file = if create_dir_all("logs").is_ok() {
        match OpenOptions::new().create(true).append(true).open(log_path) {
            Ok(file) => file,
            Err(_) => {
                log_path = "gramatical_extraction.log";
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
        log_path = "gramatical_extraction.log";
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| PpdcError::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))?
    };

    writeln!(
        log_file,
        "===== gramatical_extraction {} =====\nSYSTEM PROMPT:\n{}\n\nUSER PROMPT:\n{}\n\nOUTPUT:\n{}\n",
        Utc::now().to_rfc3339(),
        request.system_prompt,
        request.user_prompt,
        output_json
    )
    .map_err(|e| PpdcError::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))?;
    println!("Logged extraction to {}", log_path);

    Ok(())
}
