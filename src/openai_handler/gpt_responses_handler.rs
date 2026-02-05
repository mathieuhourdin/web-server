use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::info;
use crate::db;
use crate::entities_v2::llm_call::NewLlmCall;
use crate::environment;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct GPTRequest {
    model: String,
    input: Vec<GPTMessage>,
    max_output_tokens: u32,
    text: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct GPTMessage {
    role: String,
    content: String,
}

/// Shape minimal du /v1/responses qu'on veut parser
#[derive(Debug, Deserialize)]
struct GPTResponse {
    output: Vec<ResponseOutputItem>,
    status: String,
}

#[derive(Debug, Deserialize)]
struct ResponseOutputItem {
    #[serde(rename = "type")]
    kind: String, // "message", "reasoning", ...
    #[serde(default)]
    content: Vec<ContentItem>,
    // on ignore id / role / status etc., pas nécessaires pour ton use-case
}

#[derive(Debug, Deserialize)]
struct ContentItem {
    #[serde(rename = "type")]
    kind: String, // "output_text"
    text: String,
}

pub async fn make_gpt_request<T>(
    system_prompt: String,
    user_prompt: String,
    schema: Option<serde_json::Value>,
    log_header: Option<&str>,
    analysis_id: Option<Uuid>,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let api_key = environment::get_openai_api_key();
    let base_url = environment::get_openai_api_base_url();
    let request_url = format!("{}/v1/responses", base_url);
    let client = Client::new();

    let gpt_request = GPTRequest {
        model: "gpt-4.1-mini-2025-04-14".to_string(),
        input: vec![
            GPTMessage {
                role: "system".to_string(),
                content: system_prompt.clone(),
            },
            GPTMessage {
                role: "user".to_string(),
                content: user_prompt.clone(),
            },
        ],
        max_output_tokens: 4500,
        text: schema.clone().map(|schema| serde_json::json!({
            "format": {
                "type": "json_schema",
                "name": "reference_schema",
                "schema": schema.clone(),
                "strict": true
            }
        })).unwrap_or_default(),
    };

    // Serialize request to JSON string for persistence
    let request_json = serde_json::to_string(&gpt_request)
        .unwrap_or_else(|e| format!("Failed to serialize request: {e}"));
    let schema_json = serde_json::to_string(&schema)
        .unwrap_or_else(|e| format!("Failed to serialize schema: {e}"));
    let full_prompt = format!("System: {}\n\nUser: {}", system_prompt, user_prompt);

    let resp = client
        .post(&request_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&gpt_request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to GPT: {e}"))?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    // Determine call status based on HTTP status and response
    let call_status = if !status.is_success() {
        format!("error_{}", status.as_u16())
    } else {
        "completed".to_string()
    };

    // Try to parse response to extract output text (best effort before persistence)
    let output_text = serde_json::from_str::<GPTResponse>(&body)
        .ok()
        .and_then(|gpt_resp| {
            gpt_resp
                .output
                .iter()
                .find(|item| item.kind == "message")
                .and_then(|msg| {
                    msg.content
                        .iter()
                        .find(|c| c.kind == "output_text")
                        .map(|c| c.text.clone())
                })
        })
        .unwrap_or_default();

    let resolved_log_header = log_header.unwrap_or("analysis_id: unknown");
    info!(
        target: "work_analyzer",
        "{} llm_result prompt={} output={}",
        resolved_log_header,
        full_prompt,
        output_text
    );

    let env = environment::get_env();
    if env != "bintest" && analysis_id.is_some() {
        // Persist the LLM call to database before attempting full parsing
        let pool = db::get_global_pool();
        let new_call = NewLlmCall::new(
            call_status.clone(),
            "gpt-4.1-mini-2025-04-14".to_string(),
            full_prompt,
            schema_json,
            request_json,
            request_url.clone(),
            body.clone(),
            output_text.clone(),
            0, // input_tokens_used - not available in current response structure
            0, // reasoning_tokens_used - not available in current response structure
            0, // output_tokens_used - not available in current response structure
            0.0, // price - not available in current response structure
            "USD".to_string(), // currency - default
            analysis_id.unwrap(),
        );

        // Try to persist, but don't fail the whole request if persistence fails
        if let Err(e) = new_call.create(pool) {
            tracing::warn!(
                target: "work_analyzer",
                "{} llm_persist_failed error={}",
                resolved_log_header,
                e
            );
        }

        if !status.is_success() {
            return Err(format!("GPT API error ({status}): {body}").into());
        }
    }

    // Now do full parsing for type T
    let gpt_resp: GPTResponse =
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse GPTResponse: {e}"))?;

    if gpt_resp.status != "completed" {
        return Err(format!("GPT response not completed, status={}", gpt_resp.status).into());
    }

    // Extract JSON text from the parsed response
    let json_text = gpt_resp
        .output
        .iter()
        .find(|item| item.kind == "message")
        .and_then(|msg| {
            msg.content
                .iter()
                .find(|c| c.kind == "output_text")
                .map(|c| c.text.as_str())
        })
        .ok_or_else(|| "No output_text found in GPT response output".to_string())?;

    // Si pas de schéma, on parse directement le texte brut comme String
    // Sinon, on parse comme JSON selon le schéma
    let result: T = if schema.is_none() {
        // Pas de schéma : texte brut, on parse directement comme String
        serde_json::from_value(serde_json::Value::String(json_text.to_string()))
            .map_err(|e| format!("Failed to deserialize text into target type: {e}"))?
    } else {
        // Avec schéma : on parse comme JSON
        serde_json::from_str(json_text)
            .map_err(|e| format!("Failed to deserialize JSON into target type: {e}"))?
    };

    Ok(result)
}
