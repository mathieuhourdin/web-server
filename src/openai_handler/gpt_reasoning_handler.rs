use crate::environment;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct GPTRequest {
    model: String,
    input: Vec<GPTMessage>,
    max_output_tokens: u32,
    text: serde_json::Value,
    reasoning: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct GPTMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct GPTResponse {
    output: Vec<OutputItem>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum OutputItem {
    #[serde(rename = "reasoning")]
    Reasoning {
        id: String,
        summary: Vec<serde_json::Value>,
    },
    #[serde(rename = "message")]
    Message {
        content: Vec<ContentItem>,
        id: String,
        role: String,
        status: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentItem {
    #[serde(rename = "output_text")]
    OutputText {
        text: String,
        #[serde(default)]
        annotations: Vec<serde_json::Value>,
        #[serde(default)]
        logprobs: Vec<serde_json::Value>,
    },
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

    let gpt_request = GPTRequest {
        model: "gpt-5-nano-2025-08-07".to_string(),
        input: vec![
            GPTMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            GPTMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        max_output_tokens: 1800,
        reasoning: serde_json::json!({ "effort": "low" }),
        text: serde_json::json!({
            "format": {
                "type": "json_schema",
                "name": "reference_schema",
                "schema": schema
            }
        }),
    };

    let response = client
        .post(&format!("{}/v1/responses", base_url))
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
    let response_json: serde_json::Value = response.json().await?;
    println!("GPT response : {}", &response_json);

    // Extract the output array from the response
    let output_array = response_json
        .get("output")
        .and_then(|o| o.as_array())
        .ok_or_else(|| "Missing or invalid 'output' field in GPT response".to_string())?;

    // Extract the JSON text from output[].content[].text where type is "output_text"
    let mut extracted_text: Option<String> = None;
    for output_item in output_array {
        if let Some(item_type) = output_item.get("type").and_then(|t| t.as_str()) {
            if item_type == "message" {
                if let Some(content_array) = output_item.get("content").and_then(|c| c.as_array()) {
                    for content_item in content_array {
                        if let Some(content_type) = content_item.get("type").and_then(|t| t.as_str()) {
                            if content_type == "output_text" {
                                if let Some(text) = content_item.get("text").and_then(|t| t.as_str()) {
                                    extracted_text = Some(text.to_string());
                                    println!("Extracted text (raw): {:?}", text);
                                    println!("Extracted text length: {}", text.len());
                                    break;
                                }
                            }
                        }
                    }
                    if extracted_text.is_some() {
                        break;
                    }
                }
            }
        }
    }

    let text = extracted_text.ok_or_else(|| {
        "No output_text found in GPT response output".to_string()
    })?;

    // Trim whitespace and remove any potential trailing characters
    let trimmed_text = text.trim();
    println!("Trimmed extracted text: {:?}", trimmed_text);
    
    // Find the first complete JSON object by looking for matching braces
    let json_start = trimmed_text.find('{');
    let json_end = trimmed_text.rfind('}');
    
    let json_text = if let (Some(start), Some(end)) = (json_start, json_end) {
        &trimmed_text[start..=end]
    } else {
        trimmed_text
    };
    
    println!("Final JSON text to parse: {:?}", json_text);
    
    // Parse the JSON text into the desired type
    let gpt_response: T = serde_json::from_str(json_text)
        .map_err(|e| format!("Failed to parse extracted JSON '{}': {}", json_text, e))?;
    Ok(gpt_response)
}
