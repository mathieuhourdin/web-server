use crate::db;
use crate::entities_v2::llm_call::NewLlmCall;
use crate::environment;
use crate::work_analyzer::observability::format_text_log_field;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;
use uuid::Uuid;

pub const DEFAULT_OPENAI_MODEL: &str = "gpt-4.1-mini-2025-04-14";

#[derive(Clone)]
pub enum GptReasoningEffort {
    Low,
    Medium,
    High,
}

impl GptReasoningEffort {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Clone)]
pub enum GptVerbosity {
    Low,
    Medium,
    High,
}

impl GptVerbosity {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Serialize)]
struct GPTReasoning {
    effort: String,
}

#[derive(Debug, Serialize)]
struct GPTTextFormat {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    schema: serde_json::Value,
    strict: bool,
}

#[derive(Debug, Serialize)]
struct GPTText {
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<GPTTextFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verbosity: Option<String>,
}

#[derive(Debug, Serialize)]
struct GPTRequest {
    model: String,
    input: Vec<GPTMessage>,
    max_output_tokens: u32,
    text: GPTText,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<GPTReasoning>,
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
    usage: Option<ResponseUsage>,
}

#[derive(Debug, Deserialize, Default)]
struct ResponseUsage {
    input_tokens: i32,
    output_tokens: i32,
    #[serde(default)]
    input_tokens_details: InputTokenDetails,
    #[serde(default)]
    output_tokens_details: OutputTokenDetails,
}

#[derive(Debug, Deserialize, Default)]
struct InputTokenDetails {
    #[serde(default)]
    cached_tokens: i32,
}

#[derive(Debug, Deserialize, Default)]
struct OutputTokenDetails {
    #[serde(default)]
    reasoning_tokens: i32,
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

#[derive(Debug, Deserialize)]
struct ChatCompletionsResponse {
    choices: Vec<ChatCompletionChoice>,
    usage: Option<ChatCompletionsUsage>,
}

#[derive(Debug, Deserialize, Default)]
struct ChatCompletionsUsage {
    prompt_tokens: i32,
    completion_tokens: i32,
    #[serde(default)]
    prompt_tokens_details: ChatPromptTokenDetails,
    #[serde(default)]
    completion_tokens_details: ChatCompletionTokenDetails,
}

#[derive(Debug, Deserialize, Default)]
struct ChatPromptTokenDetails {
    #[serde(default)]
    cached_tokens: i32,
}

#[derive(Debug, Deserialize, Default)]
struct ChatCompletionTokenDetails {
    #[serde(default)]
    reasoning_tokens: i32,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct TokenUsage {
    input_tokens: i32,
    cached_input_tokens: i32,
    reasoning_tokens: i32,
    output_tokens: i32,
}

#[derive(Debug, Clone, Copy)]
struct ModelTokenPrices {
    input_per_million: f64,
    cached_input_per_million: f64,
    output_per_million: f64,
    long_context_threshold: Option<i32>,
}

fn extract_token_usage(body: &str) -> TokenUsage {
    if let Ok(response) = serde_json::from_str::<GPTResponse>(body) {
        return response
            .usage
            .map(|usage| TokenUsage {
                input_tokens: usage.input_tokens.max(0),
                cached_input_tokens: usage.input_tokens_details.cached_tokens.max(0),
                reasoning_tokens: usage.output_tokens_details.reasoning_tokens.max(0),
                output_tokens: usage.output_tokens.max(0),
            })
            .unwrap_or_default();
    }

    if let Ok(response) = serde_json::from_str::<ChatCompletionsResponse>(body) {
        return response
            .usage
            .map(|usage| TokenUsage {
                input_tokens: usage.prompt_tokens.max(0),
                cached_input_tokens: usage.prompt_tokens_details.cached_tokens.max(0),
                reasoning_tokens: usage.completion_tokens_details.reasoning_tokens.max(0),
                output_tokens: usage.completion_tokens.max(0),
            })
            .unwrap_or_default();
    }

    TokenUsage::default()
}

fn model_token_prices(model: &str) -> Option<ModelTokenPrices> {
    // USD per one million tokens, verified against the official OpenAI model pages on 2026-09-08.
    if model == "gpt-5.6-terra" || model.starts_with("gpt-5.6-terra-") {
        return Some(ModelTokenPrices {
            input_per_million: 2.0,
            cached_input_per_million: 0.2,
            output_per_million: 12.0,
            long_context_threshold: Some(272_000),
        });
    }
    if model == "gpt-4.1-mini" || model.starts_with("gpt-4.1-mini-") {
        return Some(ModelTokenPrices {
            input_per_million: 0.4,
            cached_input_per_million: 0.1,
            output_per_million: 1.6,
            long_context_threshold: None,
        });
    }
    None
}

fn estimate_price_usd(model: &str, usage: &TokenUsage) -> Option<f64> {
    let prices = model_token_prices(model)?;
    let cached_input_tokens = usage.cached_input_tokens.clamp(0, usage.input_tokens);
    let uncached_input_tokens = usage.input_tokens.saturating_sub(cached_input_tokens);
    let long_context = prices
        .long_context_threshold
        .is_some_and(|threshold| usage.input_tokens > threshold);
    let input_multiplier = if long_context { 2.0 } else { 1.0 };
    let output_multiplier = if long_context { 1.5 } else { 1.0 };

    Some(
        ((uncached_input_tokens as f64 * prices.input_per_million
            + cached_input_tokens as f64 * prices.cached_input_per_million)
            * input_multiplier
            + usage.output_tokens as f64 * prices.output_per_million * output_multiplier)
            / 1_000_000.0,
    )
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    content: String,
}

fn build_responses_url(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{}/responses", base)
    } else {
        format!("{}/v1/responses", base)
    }
}

fn extract_output_text(body: &str) -> Option<String> {
    if let Ok(resp) = serde_json::from_str::<GPTResponse>(body) {
        return resp
            .output
            .iter()
            .find(|item| item.kind == "message")
            .and_then(|msg| {
                msg.content
                    .iter()
                    .find(|c| c.kind == "output_text")
                    .map(|c| c.text.clone())
            });
    }

    if let Ok(resp) = serde_json::from_str::<ChatCompletionsResponse>(body) {
        return resp
            .choices
            .first()
            .map(|choice| choice.message.content.clone());
    }

    None
}

fn strip_nul_chars(input: &str) -> (String, usize) {
    let removed = input.chars().filter(|c| *c == '\0').count();
    if removed == 0 {
        return (input.to_string(), 0);
    }
    let sanitized = input.chars().filter(|c| *c != '\0').collect::<String>();
    (sanitized, removed)
}

fn sanitize_json_value_nuls(value: &mut Value) -> usize {
    match value {
        Value::String(s) => {
            let (sanitized, removed) = strip_nul_chars(s);
            if removed > 0 {
                *s = sanitized;
            }
            removed
        }
        Value::Array(items) => items.iter_mut().map(sanitize_json_value_nuls).sum(),
        Value::Object(map) => map.values_mut().map(sanitize_json_value_nuls).sum(),
        _ => 0,
    }
}

pub async fn make_gpt_request<T>(
    model: String,
    reasoning_effort: Option<GptReasoningEffort>,
    verbosity: Option<GptVerbosity>,
    system_prompt: String,
    user_prompt: String,
    schema: Option<serde_json::Value>,
    display_name: Option<&str>,
    analysis_id: Option<Uuid>,
    message_id: Option<Uuid>,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let api_key = environment::get_openai_api_key();
    let base_url = environment::get_openai_api_base_url();
    let request_url = build_responses_url(&base_url);
    let client = Client::new();

    let reasoning = reasoning_effort.map(|effort| GPTReasoning {
        effort: effort.as_str().to_string(),
    });
    let gpt_request = GPTRequest {
        model: model.clone(),
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
        max_output_tokens: 22500,
        temperature: if reasoning.is_some() { None } else { Some(0.1) },
        reasoning,
        text: GPTText {
            format: schema.clone().map(|schema| GPTTextFormat {
                kind: "json_schema".to_string(),
                name: "reference_schema".to_string(),
                schema,
                strict: true,
            }),
            verbosity: verbosity.map(|verbosity| verbosity.as_str().to_string()),
        },
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
    let output_text = extract_output_text(&body).unwrap_or_default();
    let token_usage = extract_token_usage(&body);
    let estimated_price = estimate_price_usd(&model, &token_usage).unwrap_or_else(|| {
        tracing::warn!(
            target: "work_analyzer",
            "llm_price_estimate_unavailable model={}",
            model
        );
        0.0
    });
    let (output_text, output_text_nuls_removed) = strip_nul_chars(&output_text);

    let resolved_log_header = analysis_id
        .map(|id| format!("analysis_id: {}", id))
        .unwrap_or_else(|| "analysis_id: unknown".to_string());
    info!(
        target: "work_analyzer",
        "{} llm_result name={} {} {}",
        resolved_log_header,
        display_name.unwrap_or("unknown"),
        format_text_log_field("prompt", &user_prompt),
        format_text_log_field("output", &output_text)
    );
    if output_text_nuls_removed > 0 {
        tracing::warn!(
            target: "work_analyzer",
            "{} llm_output_sanitized_nul name={} removed_nuls={}",
            resolved_log_header,
            display_name.unwrap_or("unknown"),
            output_text_nuls_removed
        );
    }

    let env = environment::get_env();
    if env != "bintest" && (analysis_id.is_some() || message_id.is_some()) {
        // Persist the LLM call to database before attempting full parsing
        let pool = db::get_global_pool();
        let new_call = NewLlmCall::new(
            call_status.clone(),
            model,
            full_prompt,
            display_name.unwrap_or("").to_string(),
            schema_json,
            request_json,
            request_url.clone(),
            body.clone(),
            output_text.clone(),
            token_usage.input_tokens,
            token_usage.reasoning_tokens,
            token_usage.output_tokens,
            estimated_price,
            "USD".to_string(),
            analysis_id,
            system_prompt,
            user_prompt,
            message_id,
            token_usage.cached_input_tokens,
        );

        // Try to persist, but don't fail the whole request if persistence fails
        if let Err(e) = new_call.create(pool) {
            tracing::warn!(
                target: "work_analyzer",
                "{} llm_persist_failed name={} error={}",
                resolved_log_header,
                display_name.unwrap_or("unknown"),
                e
            );
        }
    }

    if !status.is_success() {
        return Err(format!("GPT API error ({status}): {body}").into());
    }

    let json_text = if let Ok(gpt_resp) = serde_json::from_str::<GPTResponse>(&body) {
        if gpt_resp.status != "completed" {
            return Err(format!("GPT response not completed, status={}", gpt_resp.status).into());
        }
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
            .ok_or_else(|| "No output_text found in GPT response output".to_string())?
    } else if let Ok(chat_resp) = serde_json::from_str::<ChatCompletionsResponse>(&body) {
        chat_resp
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or_else(|| "No choices[0].message.content found in GPT response".to_string())?
    } else {
        return Err(format!("Failed to parse GPT response body: {}", body).into());
    };

    let (json_text, removed_nuls_in_json_text) = strip_nul_chars(&json_text);
    if removed_nuls_in_json_text > 0 {
        tracing::warn!(
            target: "work_analyzer",
            "{} llm_json_text_sanitized_nul name={} removed_nuls={}",
            resolved_log_header,
            display_name.unwrap_or("unknown"),
            removed_nuls_in_json_text
        );
    }

    // Si pas de schéma, on parse directement le texte brut comme String.
    // Sinon, on parse comme JSON puis on nettoie les NUL dans tous les champs texte.
    let result: T = if schema.is_none() {
        serde_json::from_value(Value::String(json_text))
            .map_err(|e| format!("Failed to deserialize text into target type: {e}"))?
    } else {
        let mut json_value: Value = serde_json::from_str(&json_text)
            .map_err(|e| format!("Failed to deserialize JSON text into Value: {e}"))?;
        let removed_nuls_in_json_value = sanitize_json_value_nuls(&mut json_value);
        if removed_nuls_in_json_value > 0 {
            tracing::warn!(
                target: "work_analyzer",
                "{} llm_json_value_sanitized_nul name={} removed_nuls={}",
                resolved_log_header,
                display_name.unwrap_or("unknown"),
                removed_nuls_in_json_value
            );
        }
        serde_json::from_value(json_value)
            .map_err(|e| format!("Failed to deserialize JSON into target type: {e}"))?
    };

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{
        estimate_price_usd, extract_token_usage, sanitize_json_value_nuls, strip_nul_chars,
        TokenUsage,
    };
    use serde_json::json;

    #[test]
    fn strip_nul_chars_removes_nuls_only() {
        let (sanitized, removed) = strip_nul_chars("ab\0cd");
        assert_eq!(sanitized, "abcd");
        assert_eq!(removed, 1);
    }

    #[test]
    fn sanitize_json_value_nuls_removes_nuls_recursively() {
        let mut value = json!({
            "title": "d\0e9ni",
            "tags": ["toxicit\0e9", "ok"],
            "nested": { "content": "psychoth\0e9rapie" }
        });
        let removed = sanitize_json_value_nuls(&mut value);
        assert_eq!(removed, 3);
        assert_eq!(value["title"], "de9ni");
        assert_eq!(value["tags"][0], "toxicite9");
        assert_eq!(value["nested"]["content"], "psychothe9rapie");
    }

    #[test]
    fn extracts_responses_api_token_usage() {
        let body = json!({
            "status": "completed",
            "output": [],
            "usage": {
                "input_tokens": 12_000,
                "input_tokens_details": { "cached_tokens": 2_000 },
                "output_tokens": 800,
                "output_tokens_details": { "reasoning_tokens": 300 },
                "total_tokens": 12_800
            }
        })
        .to_string();

        assert_eq!(
            extract_token_usage(&body),
            TokenUsage {
                input_tokens: 12_000,
                cached_input_tokens: 2_000,
                reasoning_tokens: 300,
                output_tokens: 800,
            }
        );
    }

    #[test]
    fn estimates_terra_price_without_double_counting_reasoning_tokens() {
        let usage = TokenUsage {
            input_tokens: 12_000,
            cached_input_tokens: 2_000,
            reasoning_tokens: 300,
            output_tokens: 800,
        };

        let price = estimate_price_usd("gpt-5.6-terra", &usage).unwrap();
        assert!((price - 0.03).abs() < 1e-12);
    }

    #[test]
    fn applies_terra_long_context_multiplier() {
        let usage = TokenUsage {
            input_tokens: 300_000,
            cached_input_tokens: 0,
            reasoning_tokens: 1_000,
            output_tokens: 10_000,
        };

        let price = estimate_price_usd("gpt-5.6-terra", &usage).unwrap();
        assert!((price - 1.38).abs() < 1e-12);
    }
}
