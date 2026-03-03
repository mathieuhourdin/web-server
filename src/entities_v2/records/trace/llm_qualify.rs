use crate::entities::error::PpdcError;
use crate::openai_handler::gpt_responses_handler::make_gpt_request;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceExtractionProperties {
    pub title: String,
    pub subtitle: String,
    pub interaction_date: Option<NaiveDate>,
}

pub async fn qualify_trace(trace: &str) -> Result<TraceExtractionProperties, PpdcError> {
    let system_prompt = "System:
Tu es un asistant de prise de notes et de journaling.
Détermine un titre et un sous titre en français pour le texte écrit par un utilisateur.
Si présent, détermine aussi une date où la prise de note a eu lieu.
Réponds uniquement avec du JSON valide respectant le schéma donné.";

    let user_prompt = format!(
        "User:
Basé sur la transcription audio suivante, extrais uniquement les champs:
- title (string)
- subtitle (string)
- interaction_date (datetime, optional)

Contenu : {}\n\n",
        trace
    );

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "subtitle": {"type": "string"},
            "interaction_date": {
                "anyOf": [
                    {"type": "string", "format": "date"},
                    {"type": "null"}
                ]
            }
        },
        "required": ["title", "subtitle", "interaction_date"],
        "additionalProperties": false
    });

    let result = make_gpt_request(
        system_prompt.to_string(),
        user_prompt,
        Some(schema),
        Some("Trace / Qualification"),
        None,
    )
    .await?;
    Ok(result)
}
