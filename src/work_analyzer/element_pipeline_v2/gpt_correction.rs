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
    repair_hint: Option<String>,
}

pub async fn correct_extraction(
    analysis_id: Uuid,
    trace_text: &str,
    previous_system_prompt: String,
    previous_user_prompt: String,
    previous_output: GrammaticalExtractionOutput,
) -> Result<GrammaticalExtractionOutput, PpdcError> {
    let repair_hint = build_done_repair_hint(trace_text, &previous_output);
    let correction_system_prompt = include_str!("correction_system.md").to_string();
    let correction_user_prompt = serde_json::to_string_pretty(&GrammaticalCorrectionPromptInput {
        previous_system_prompt,
        previous_user_prompt,
        previous_output,
        repair_hint,
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

fn build_done_repair_hint(
    trace_text: &str,
    extraction: &GrammaticalExtractionOutput,
) -> Option<String> {
    let normalized = trace_text.to_lowercase();
    let activity_markers = [
        "j'ai fait",
        "j’ai fait",
        "j ai fait",
        "j'ai commencé",
        "j’ai commencé",
        "j ai commencé",
        "j'ai terminé",
        "j’ai terminé",
        "j ai terminé",
        "j'ai avancé",
        "j’ai avancé",
        "j ai avancé",
        "j'ai travaillé",
        "j’ai travaillé",
        "j ai travaillé",
        "j'ai lu",
        "j’ai lu",
        "j ai lu",
        "j'ai écrit",
        "j’ai écrit",
        "j ai écrit",
        "j'ai regardé",
        "j’ai regardé",
        "j ai regardé",
        "j'ai écouté",
        "j’ai écouté",
        "j ai écouté",
        "j'ai testé",
        "j’ai testé",
        "j ai testé",
        "j'ai codé",
        "j’ai codé",
        "j ai codé",
        "i've done",
        "i have done",
        "i did",
        "i started",
        "i've started",
        "i have started",
        "i began",
        "i worked on",
        "i read",
        "i wrote",
        "i watched",
        "i listened",
        "i tested",
        "i coded",
        "i finished",
        "i've finished",
        "i have finished",
        "i completed",
    ];
    let detected_marker = activity_markers
        .iter()
        .find(|marker| normalized.contains(**marker))
        .copied();

    let Some(detected_marker) = detected_marker else {
        return None;
    };

    let has_done_transaction = extraction
        .transactions
        .iter()
        .any(|transaction| transaction.status.eq_ignore_ascii_case("DONE"));

    if has_done_transaction {
        return None;
    }

    Some(
        format!(
            "Potential inconsistency detected: the trace contains an explicit user activity marker \
            \"{}\" but the extraction has no transaction with status DONE. Please repair the output \
            by marking clearly performed user transaction as DONE when justified by the trace.",
            detected_marker
        ),
    )
}
