use regex::{self, Regex};
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
    repair_hint: Option<String>,
}

pub async fn correct_references_drafts(
    analysis_id: Uuid,
    previous_system_prompt: String,
    previous_user_prompt: String,
    previous_output: ReferencesExtractionResult,
) -> Result<ReferencesExtractionResult, PpdcError> {
    let repair_hint = build_date_extraction_repair_hint(&previous_output);
    let correction_system_prompt = include_str!("correction_system.md").to_string();
    let correction_prompt_input = ReferencesCorrectionPromptInput {
        previous_system_prompt,
        previous_user_prompt,
        previous_output,
        repair_hint,
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

fn build_date_extraction_repair_hint(
    extraction: &ReferencesExtractionResult,
) -> Option<String> {
    let mut date_like_mentions = extraction
        .references
        .iter()
        .map(|reference| reference.mention.trim())
        .filter(|mention| !mention.is_empty() && looks_like_date_mention(mention))
        .map(|mention| mention.to_string())
        .collect::<Vec<_>>();

    date_like_mentions.sort();
    date_like_mentions.dedup();

    if date_like_mentions.is_empty() {
        return None;
    }

    Some(format!(
        "Potential false-positive date references detected in the extracted output: {}. \
        Please remove date/time expressions from references when they are dates rather than \
        entities.",
        date_like_mentions
            .into_iter()
            .map(|mention| format!("\"{}\"", mention))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn looks_like_date_mention(mention: &str) -> bool {
    let normalized = mention.to_lowercase();

    // Inspired by `src/bin/import_txt.rs` date parsing patterns.
    let iso_date_time_regex = Regex::new(
        r"\b\d{4}-\d{2}-\d{2}(?:[ t]\d{1,2}(?::\d{2}|h\d{2}))?\b",
    )
    .expect("valid regex");
    let slash_or_dash_numeric_date_regex =
        Regex::new(r"\b\d{1,2}[/-]\d{1,2}(?:[/-]\d{2,4})?\b").expect("valid regex");

    let days_fr = [
        "lundi",
        "mardi",
        "mercredi",
        "jeudi",
        "vendredi",
        "samedi",
        "dimanche",
    ];
    let months_fr = [
        "janvier",
        "février",
        "fevrier",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "aout",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
        "decembre",
    ];
    let days_pattern = days_fr
        .iter()
        .map(|d| regex::escape(d))
        .collect::<Vec<String>>()
        .join("|");
    let months_pattern = months_fr
        .iter()
        .map(|m| regex::escape(m))
        .collect::<Vec<String>>()
        .join("|");
    let french_long_date_pattern = format!(
        r"\b(?:{})\s+\d{{1,2}}(?:er)?\s+(?:{})(?:\s+\d{{4}})?(?:\s+\d{{1,2}}(?:h|:)\d{{2}})?\b",
        days_pattern, months_pattern
    );
    let french_long_date_regex =
        Regex::new(french_long_date_pattern.as_str()).expect("valid regex");

    let relative_date_regex = Regex::new(
        r"\b(today|yesterday|tomorrow|tonight|ce matin|cet après-midi|cet apres-midi|ce soir|aujourd'hui|aujourd’hui|hier|demain)\b",
    )
    .expect("valid regex");

    iso_date_time_regex.is_match(&normalized)
        || slash_or_dash_numeric_date_regex.is_match(&normalized)
        || french_long_date_regex.is_match(&normalized)
        || relative_date_regex.is_match(&normalized)
}
