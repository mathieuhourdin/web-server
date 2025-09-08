use crate::environment;
use reqwest::Client;
use serde::{Serialize, Deserialize};
//use crate::entities::process_audio::process_audio_routes::{GPTRequest, GPTMessage, GPTResponse, ExtractionProperties, GenerationProperties};

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

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractionProperties {
    pub titre: String,
    pub auteur: String,
    pub projet: String,
    pub commentaire: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerationProperties {
    pub ressource_summary: String,
    pub estimated_publishing_date: String,
    pub interaction_comment: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Properties {
    pub extraction_props: ExtractionProperties,
    pub generation_props: GenerationProperties,
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
pub async fn generate_resource_info_with_gpt(transcription: &str) -> Result<Properties, Box<dyn std::error::Error>> {
    let extraction_props = extract_resource_info_with_gpt(transcription).await?;
    let generation_props = generate_resource_summary_with_gpt(&extraction_props.titre, &extraction_props.auteur, &extraction_props.projet, &extraction_props.commentaire).await?;

    let properties = Properties {
        extraction_props,
        generation_props,
    };

    Ok(properties)
}

async fn extract_resource_info_with_gpt(transcription: &str) -> Result<ExtractionProperties, Box<dyn std::error::Error>> {
    // Get OpenAI API key and base URL from environment
    let api_key = environment::get_openai_api_key();
    let base_url = environment::get_openai_api_base_url();
    
    let client = Client::new();

    let system_prompt = format!("System:
    Tu es un expert en extraction de ressources académiques et littéraires.
    Corrige activement les erreurs de transcription dans les noms propres. 
    Si un auteur ou un titre ressemble fortement à un auteur/ouvrage connu, propose la version corrigée.
    Exemple : 'Raymond Gruyère' → 'Raymond Ruyer'.
    Réponds uniquement avec du JSON valide respectant le schéma donné.
    Si une information est trop incertaine ou absente, mets NA.
    Ne fais pas de résumé à ce stade.
    ");

    let user_prompt = format!("User:
    Basé sur la transcription audio suivante, extrais uniquement les champs:
    - titre (string)
    - auteur (string)
    - projet (string)
    - commentaire (string)

    Transcription: {}\n\n", transcription);

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
                        "titre": {"type": "string"},
                        "auteur": {"type": "string"},
                        "projet": {"type": "string"},
                        "commentaire": {"type": "string"}
                    },
                    "required": ["titre", "auteur"]
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
    let extraction_props: ExtractionProperties = serde_json::from_str(&json_content)
        .map_err(|e| format!("Failed to parse GPT JSON response: {}", e))?;

    Ok(extraction_props)
}   

async fn generate_resource_summary_with_gpt(title: &str, author_name: &str, _project: &str, _comment: &str) -> Result<GenerationProperties, Box<dyn std::error::Error>> {
    let api_key = environment::get_openai_api_key();
    let base_url = environment::get_openai_api_base_url();
    
    let client = Client::new();
    
    let system_prompt = format!("System:
    Tu es un expert en sciences humaines et sociales. 
    À partir d’un titre et d’un auteur fournis, rédige un résumé détaillé (200 à 300 mots, en français).
    Le résumé doit expliquer les idées principales, les découvertes clés et la signification de l’ouvrage.");

    let user_prompt = format!("User:
    Rédige un résumé détaillé pour l’ouvrage suivant:
    - Titre: {title}
    - Auteur: {author_name}

    ");


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
        temperature: 0.8,
        response_format: serde_json::json!({
            "type": "json_schema",
            "json_schema": {
                "name": "reference_schema",
                "schema": {
                    "type": "object",
                    "properties": {
                        "ressource_summary": {"type": "string"},
                        "estimated_publishing_date": {"type": "string"},
                        "interaction_comment": {"type": "string"}
                    },
                    "required": ["ressource_summary", "estimated_publishing_date", "interaction_comment"]
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
    
    // Parse the JSON response into GenerationProperties
    let generation_props: GenerationProperties = serde_json::from_str(&json_content)
        .map_err(|e| format!("Failed to parse GPT JSON response: {}", e))?;

    Ok(generation_props)
}