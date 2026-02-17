use crate::environment;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct GPTRequest {
    model: String,
    messages: Vec<GPTMessage>,
    max_tokens: u32,
    temperature: f32,
    response_format: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct GPTMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct GPTResponse {
    choices: Vec<GPTChoice>,
}

#[derive(Debug, Deserialize)]
struct GPTChoice {
    message: GPTMessageResponse,
}

#[derive(Debug, Deserialize)]
struct GPTMessageResponse {
    content: String,
}

pub async fn make_gpt_request<T>(
    system_prompt: String,
    user_prompt: String,
    schema: serde_json::Value,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    // Get OpenAI API key and base URL from environment
    let api_key = environment::get_openai_api_key();
    let base_url = environment::get_openai_api_base_url();

    let client = Client::new();

    let metadata = format!(
        "Date of today: {}",
        Utc::now().format("%Y-%m-%d").to_string()
    );

    let system_prompt_with_metadata = format!(
        "{}\n\n
    Metadata:\n{}",
        system_prompt, metadata
    );

    let gpt_request = GPTRequest {
        model: "gpt-4.1-nano-2025-04-14".to_string(),
        messages: vec![
            GPTMessage {
                role: "system".to_string(),
                content: system_prompt_with_metadata,
            },
            GPTMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        max_tokens: 1800,
        temperature: 0.1,
        response_format: serde_json::json!({
            "type": "json_schema",
            "json_schema": {
                "name": "reference_schema",
                "schema": schema
            }
        }),
    };

    let response = client
        .post(&format!("{}/v1/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&gpt_request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to GPT: {}", e))?;

    let status = response.status();

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("GPT API error ({}): {}", status, error_text).into());
    }

    let gpt_response: GPTResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GPT response: {}", e))?;

    // Extract the JSON content from the response
    let json_content = gpt_response
        .choices
        .first()
        .and_then(|choice| Some(choice.message.content.clone()))
        .ok_or("No response content from GPT")?;

    // Parse the JSON response into ExtractionProperties
    println!("GPT response : {}", json_content);
    Ok(serde_json::from_str(&json_content)
        .map_err(|e| format!("Failed to parse GPT JSON response: {}", e))?)
}
