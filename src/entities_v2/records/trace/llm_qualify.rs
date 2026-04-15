use crate::entities_v2::error::PpdcError;
use crate::openai_handler::gpt_responses_handler::{make_gpt_request, DEFAULT_OPENAI_MODEL};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceExtractionProperties {
    pub title: String,
    pub subtitle: String,
}

pub async fn qualify_trace(trace: &str) -> Result<TraceExtractionProperties, PpdcError> {
    let system_prompt = "System:
Tu es un asistant de prise de notes et de journaling.
Détermine un titre et un sous titre en français pour le texte écrit par un utilisateur.
Réponds uniquement avec du JSON valide respectant le schéma donné.";

    let user_prompt = format!(
        "User:
Basé sur la transcription audio suivante, extrais uniquement les champs:
- title (string)
- subtitle (string)

Contenu : {}\n\n",
        trace
    );

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "subtitle": {"type": "string"}
        },
        "required": ["title", "subtitle"],
        "additionalProperties": false
    });

    let result = make_gpt_request(
        DEFAULT_OPENAI_MODEL.to_string(),
        None,
        None,
        system_prompt.to_string(),
        user_prompt,
        Some(schema),
        Some("Trace / Qualification"),
        None,
    )
    .await?;
    Ok(result)
}
