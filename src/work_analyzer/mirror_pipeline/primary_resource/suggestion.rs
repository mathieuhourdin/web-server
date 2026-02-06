use serde::{Deserialize, Serialize};
use crate::entities_v2::trace::Trace;
use uuid::Uuid;
use crate::entities::error::{PpdcError, ErrorType};
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::matching::ElementWithIdentifier;

const MAX_RETRIES: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryResourceSuggestion {
    pub resource_identifier: String,
    pub theme: Option<String>,
    pub author: Option<String>,
    pub evidence: Vec<String>,
}

impl ElementWithIdentifier for PrimaryResourceSuggestion {
    fn identifier(&self) -> String {
        self.resource_identifier.clone()
    }
}

/// Returns the evidence strings that are not exact substrings of `trace_content`.
fn invalid_evidence(evidence: &[String], trace_content: &str) -> Vec<String> {
    evidence
        .iter()
        .filter(|s| !trace_content.contains(s.as_str()))
        .cloned()
        .collect()
}

pub async fn extract(trace: &Trace, analysis_id: Uuid, log_header: &str) -> Result<PrimaryResourceSuggestion, PpdcError> {
    let system_prompt = include_str!("../prompts/primary_resource/suggestion/system.md").to_string();
    let schema_str = include_str!("../prompts/primary_resource/suggestion/schema.json");
    let schema: serde_json::Value = serde_json::from_str(schema_str)?;
    let trace_content = trace.content.clone();
    let mut last_invalid: Option<Vec<String>> = None;
    let mut retry_count = 0u32;

    loop {
        let user_prompt = match &last_invalid {
            None => format!("trace_text :\n{}\n", trace_content),
            Some(invalid) => format!(
                "trace_text :\n{}\n\n\
                Erreur de validation : les chaînes suivantes dans 'evidence' ne sont pas des sous-chaînes exactes de trace_text. \
                Chaque élément de evidence doit être copié mot pour mot depuis trace_text, sans reformulation. \
                Chaînes invalides (à corriger ou remplacer par des extraits littéraux) : {:?}. \
                Réponds à nouveau avec des evidence valides.",
                trace_content, invalid
            ),
        };

        let config = GptRequestConfig::new(
            "gpt-4.1-mini".to_string(),
            system_prompt.clone(),
            user_prompt,
            Some(schema.clone()),
            Some(analysis_id),
        )
        .with_log_header(log_header)
        .with_display_name("Mirror / Primary Resource Suggestion");
        let result: PrimaryResourceSuggestion = config.execute().await?;

        let invalid = invalid_evidence(&result.evidence, &trace_content);
        if invalid.is_empty() {
            return Ok(result);
        }

        last_invalid = Some(invalid.clone());
        retry_count += 1;
        if retry_count >= MAX_RETRIES {
            return Err(PpdcError::new(
                422,
                ErrorType::ApiError,
                format!(
                    "Evidence validation failed after {} retries: the following strings are not substrings of the trace content: {:?}",
                    MAX_RETRIES, invalid
                ),
            ));
        }
    }
}
