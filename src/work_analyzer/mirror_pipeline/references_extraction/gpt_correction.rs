use serde::Serialize;
use uuid::Uuid;

use crate::entities::error::PpdcError;
use crate::openai_handler::GptRequestConfig;

use super::gpt_request::{references_extraction_schema, ReferencesExtractionResult};

#[derive(Debug, Serialize)]
struct ReferencesCorrectionPromptInput {
    previous_system_prompt: String,
    previous_user_prompt: String,
    previous_output: ReferencesExtractionResult,
}

pub async fn correct_references_drafts(
    analysis_id: Uuid,
    previous_system_prompt: String,
    previous_user_prompt: String,
    previous_output: ReferencesExtractionResult,
) -> Result<ReferencesExtractionResult, PpdcError> {
    let correction_system_prompt = include_str!("correction_system.md").to_string();
    let correction_prompt_input = ReferencesCorrectionPromptInput {
        previous_system_prompt,
        previous_user_prompt,
        previous_output,
    };
    let correction_user_prompt = serde_json::to_string_pretty(&correction_prompt_input)?;
    let schema = references_extraction_schema()?;

    let config = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        correction_system_prompt,
        correction_user_prompt,
        Some(schema),
        Some(analysis_id),
    )
    .with_display_name("Mirror / References Extraction Correction");

    config.execute().await
}
