use serde::Serialize;
use uuid::Uuid;

use crate::entities::error::PpdcError;
use crate::openai_handler::GptRequestConfig;

use super::gpt_request::{grammatical_extraction_schema, GrammaticalExtractionOutput};

#[derive(Debug, Serialize)]
struct GrammaticalCorrectionPromptInput {
    previous_system_prompt: String,
    previous_user_prompt: String,
    previous_output: GrammaticalExtractionOutput,
}

pub async fn correct_extraction(
    analysis_id: Uuid,
    previous_system_prompt: String,
    previous_user_prompt: String,
    previous_output: GrammaticalExtractionOutput,
) -> Result<GrammaticalExtractionOutput, PpdcError> {
    let correction_system_prompt = include_str!("correction_system.md").to_string();
    let correction_user_prompt = serde_json::to_string_pretty(&GrammaticalCorrectionPromptInput {
        previous_system_prompt,
        previous_user_prompt,
        previous_output,
    })?;
    let schema = grammatical_extraction_schema()?;

    let request = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        correction_system_prompt,
        correction_user_prompt,
        Some(schema),
        Some(analysis_id),
    )
    .with_display_name("Element Pipeline V2 / Grammatical Extraction Correction");

    request.execute().await
}
