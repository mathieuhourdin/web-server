use crate::environment;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use crate::entities::interaction::model::InteractionWithResource;


#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    file: String,
    response_format: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    text: String,
}

#[derive(Debug, Serialize)]
struct GPTRequest {
    model: String,
    messages: Vec<GPTMessage>,
    max_tokens: u32,
    temperature: f32,
    response_format: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct GPTResponseFormat {
    #[serde(rename = "type")]
    response_type: String,
    json_schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct GPTMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct GPTResponse {
    choices: Vec<GPTChoice>,
}

#[derive(Debug, Deserialize)]
struct GPTChoice {
    message: GPTMessageResponse,
}

#[derive(Debug, Deserialize)]
struct GPTMessageResponse {
    content: String,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceExtractionProperties {
    pub title: String,
    pub subtitle: String
}

pub async fn extract_user_input_resource_info_with_gpt(content: &str) -> Result<ResourceExtractionProperties, Box<dyn std::error::Error>> {
    // Get OpenAI API key and base URL from environment
    let api_key = environment::get_openai_api_key();
    let base_url = environment::get_openai_api_base_url();
    
    let client = Client::new();

    let system_prompt = format!("System:
    Tu es un asistant de prise de notes et de journaling.
    Détermine un titre et un sous titre en français pour le texte écrit par un utilisateur.
    Indique des éléments de contexte s'ils sont disponibles, tels que la date, le type (journaling, prise de note),
    Une oeuvre ou un projet sur lequel porte le contenu fourni.
    Réponds uniquement avec du JSON valide respectant le schéma donné.
    ");

    let user_prompt = format!("User:
    Basé sur la transcription audio suivante, extrais uniquement les champs:
    - title (string)
    - subtitle (string)

    Contenu : {}\n\n", content);

    let gpt_request = GPTRequest {
        model: "gpt-4o".to_string(),
        messages: vec![
            GPTMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            GPTMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        max_tokens: 800,
        temperature: 0.5,
        response_format: serde_json::json!({
            "type": "json_schema",
            "json_schema": {
                "name": "reference_schema",
                "schema": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"},
                        "subtitle": {"type": "string"},
                    },
                    "required": ["title", "subtitle"]
                }
            }
        }),
    };

    let response = client
        .post(&format!("{}/v1/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&gpt_request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to GPT: {}", e))?;
        
    let status = response.status();
    
    if !status.is_success() {
        let error_text = response.text().await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("GPT API error ({}): {}", status, error_text).into());
    }
    
    let gpt_response: GPTResponse = response.json().await
        .map_err(|e| format!("Failed to parse GPT response: {}", e))?;

    // Extract the JSON content from the response
    let json_content = gpt_response.choices
        .first()
        .and_then(|choice| Some(choice.message.content.clone()))
        .ok_or("No response content from GPT")?;
    
    // Parse the JSON response into ExtractionProperties
    let extraction_props: ResourceExtractionProperties = serde_json::from_str(&json_content)
        .map_err(|e| format!("Failed to parse GPT JSON response: {}", e))?;

    Ok(extraction_props)
}
