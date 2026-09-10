use base64::{engine::general_purpose::STANDARD, Engine};
use google_cloud_auth::credentials::Builder as CredentialsBuilder;
use serde_json::{json, Value};

use crate::{
    entities_v2::{
        error::{ErrorType, PpdcError},
        platform_infra::asset::{download_asset_bytes_from_gcs, Asset},
    },
    environment,
};

const DOCUMENT_AI_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

pub struct PipelineResult {
    pub challenges: Value,
    pub estimated_cost_usd: f64,
}

pub async fn enrich(
    canonical: &str,
    assets: &[Asset],
    canonical_usage: &Value,
) -> Result<PipelineResult, PpdcError> {
    let (ocr_text, low_confidence, pages) = google_ocr(assets).await?;
    let prompt = format!("You are an uncertainty adjudicator, not a re-transcriber. You do not have the images. GPT-4.1 is the canonical reading. Compare it with Google OCR and report only important disagreements or low-confidence readings. Return at most 12 concise challenges and at most 3 alternatives each.\n\nGOOGLE OCR:\n{ocr_text}\n\nGPT-4.1 CANONICAL:\n{canonical}\n\nLOW CONFIDENCE OCR TOKENS:\n{}", low_confidence.join("\n"));
    let schema = json!({"type":"object","additionalProperties":false,"required":["challenges"],"properties":{"challenges":{"type":"array","maxItems":12,"items":{"type":"object","additionalProperties":false,"required":["page_number","transcribed_text","surrounding_context","confidence","alternatives","reason"],"properties":{"page_number":{"type":"integer","minimum":1},"transcribed_text":{"type":"string","maxLength":120},"surrounding_context":{"type":"string","maxLength":240},"confidence":{"type":"string","enum":["low","medium"]},"alternatives":{"type":"array","maxItems":3,"items":{"type":"string","maxLength":120}},"reason":{"type":"string","maxLength":240}}}}}});
    let response = openai(json!({"model":"gpt-5.6-luna","reasoning":{"effort":"low"},"input":[{"role":"user","content":prompt}],"text":{"format":{"type":"json_schema","name":"handwriting_challenges","strict":true,"schema":schema}},"store":false,"max_output_tokens":4000})).await?;
    let text =
        output_text(&response).ok_or_else(|| err("GPT-5.6 Luna returned no adjudication"))?;
    let challenges = serde_json::from_str::<Value>(&text)
        .map_err(|e| err(&format!("Invalid adjudication JSON: {e}")))?
        .get("challenges")
        .cloned()
        .unwrap_or(json!([]));
    let cost = openai_cost(canonical_usage, 0.40, 1.60)
        + openai_cost(response.get("usage").unwrap_or(&Value::Null), 0.25, 1.25)
        + pages as f64 * 0.0015;
    Ok(PipelineResult {
        challenges,
        estimated_cost_usd: cost,
    })
}

async fn google_ocr(assets: &[Asset]) -> Result<(String, Vec<String>, usize), PpdcError> {
    let creds = CredentialsBuilder::default()
        .with_scopes([DOCUMENT_AI_SCOPE])
        .build_access_token_credentials()
        .map_err(|e| err(&format!("Google credentials: {e}")))?;
    let token = creds
        .access_token()
        .await
        .map_err(|e| err(&format!("Google token: {e}")))?;
    let location = environment::get_google_document_ai_location();
    let endpoint = format!(
        "https://{}-documentai.googleapis.com/v1/projects/{}/locations/{}/processors/{}:process",
        location,
        environment::get_google_cloud_project(),
        location,
        environment::get_google_document_ai_processor_id()
    );
    let mut texts = Vec::new();
    let mut low = Vec::new();
    for (index, asset) in assets.iter().enumerate() {
        let bytes = download_asset_bytes_from_gcs(&asset.bucket, &asset.object_key).await?;
        let response=reqwest::Client::new().post(&endpoint).bearer_auth(&token.token).json(&json!({"rawDocument":{"content":STANDARD.encode(bytes),"mimeType":asset.mime_type},"processOptions":{"ocrConfig":{"hints":{"languageHints":["fr"]}}}})).send().await.map_err(|e| err(&format!("Google Document AI request: {e}")))?;
        let status = response.status();
        let body = response.text().await.map_err(|e| err(&e.to_string()))?;
        if !status.is_success() {
            return Err(err(&format!("Google Document AI HTTP {status}: {body}")));
        }
        let value: Value = serde_json::from_str(&body).map_err(|e| err(&e.to_string()))?;
        let document = &value["document"];
        let text = document["text"].as_str().unwrap_or_default();
        texts.push(text.trim().to_string());
        if let Some(tokens) = document["pages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["tokens"].as_array())
            .flatten()
            .collect::<Vec<_>>()
            .into_iter()
            .next()
            .map(|_| document["pages"].as_array().unwrap())
        {
            for token in tokens
                .iter()
                .filter_map(|p| p["tokens"].as_array())
                .flatten()
            {
                let confidence = token["layout"]["confidence"].as_f64().unwrap_or(0.0);
                if confidence < 0.8 {
                    low.push(format!(
                        "page {}: confidence {:.0}%",
                        index + 1,
                        confidence * 100.0
                    ));
                }
            }
        }
    }
    Ok((texts.join("\n\n"), low, assets.len()))
}

async fn openai(body: Value) -> Result<Value, PpdcError> {
    let base = environment::get_openai_api_base_url();
    let url = if base.trim_end_matches('/').ends_with("/v1") {
        format!("{}/responses", base.trim_end_matches('/'))
    } else {
        format!("{}/v1/responses", base.trim_end_matches('/'))
    };
    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(environment::get_openai_api_key())
        .json(&body)
        .send()
        .await
        .map_err(|e| err(&e.to_string()))?;
    let status = response.status();
    let text = response.text().await.map_err(|e| err(&e.to_string()))?;
    if !status.is_success() {
        return Err(err(&format!("OpenAI adjudication HTTP {status}: {text}")));
    }
    serde_json::from_str(&text).map_err(|e| err(&e.to_string()))
}
fn output_text(v: &Value) -> Option<String> {
    v.get("output_text")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            v["output"]
                .as_array()?
                .iter()
                .filter_map(|i| i["content"].as_array())
                .flatten()
                .find_map(|p| p["text"].as_str())
                .map(str::to_owned)
        })
}
fn openai_cost(v: &Value, input: f64, output: f64) -> f64 {
    v["input_tokens"].as_f64().unwrap_or(0.0) * input / 1_000_000.0
        + v["output_tokens"].as_f64().unwrap_or(0.0) * output / 1_000_000.0
}
fn err(message: &str) -> PpdcError {
    PpdcError::new(502, ErrorType::InternalError, message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_responses_text() {
        let value = json!({"output":[{"content":[{"type":"output_text","text":"hello"}]}]});
        assert_eq!(output_text(&value).as_deref(), Some("hello"));
    }

    #[test]
    fn computes_openai_cost() {
        let usage = json!({"input_tokens":1_000_000,"output_tokens":1_000_000});
        assert!((openai_cost(&usage, 0.4, 1.6) - 2.0).abs() < f64::EPSILON);
    }
}
