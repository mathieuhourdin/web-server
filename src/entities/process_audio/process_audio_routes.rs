

use axum::{
    extract::{Extension, Multipart},
    Json,
};
use serde::{Deserialize, Serialize};
use crate::entities::{session::Session, error::{PpdcError, ErrorType}};
use crate::db::DbPool;
use crate::environment;
use uuid::Uuid;
use std::path::Path;
use reqwest::Client;
use chrono::Utc;
use crate::entities::resource::{Resource, NewResource};
use crate::entities::resource::resource_type::ResourceType;
use crate::entities::resource::maturing_state::MaturingState;
use crate::entities::interaction::model::{Interaction, NewInteraction, InteractionWithResource};
use diesel::RunQueryDsl;

#[derive(Serialize)]
pub struct ProcessAudio {
    pub message: String,
    pub filename: Option<String>,
    pub transcription: Option<String>,
    pub resource_info: Option<ResourceInfo>,
    pub created_interaction: Option<InteractionWithResource>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub title: String,
    pub author: String,
    pub project: String,
    pub interaction_comment: String,
    pub resource_summary: String,
    pub estimated_publishing_date: String,
}

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
    response_format: GPTResponseFormat,
}

#[derive(Debug, Serialize)]
struct GPTResponseFormat {
    #[serde(rename = "type")]
    response_type: String,
    json_schema: JsonSchema,
}

#[derive(Debug, Serialize)]
struct JsonSchema {
    name: String,
    schema: SchemaDefinition,
}

#[derive(Debug, Serialize)]
struct SchemaDefinition {
    #[serde(rename = "type")]
    schema_type: String,
    properties: Properties,
    required: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Properties {
    title: PropertyDefinition,
    author: PropertyDefinition,
    project: PropertyDefinition,
    interaction_comment: PropertyDefinition,
    resource_summary: PropertyDefinition,
    estimated_publishing_date: PropertyDefinition,
}

#[derive(Debug, Serialize)]
struct PropertyDefinition {
    #[serde(rename = "type")]
    property_type: String,
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

// This route receives audio files via multipart/form-data and transcribes them using OpenAI
pub async fn post_process_audio_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    mut multipart: Multipart,
) -> Result<Json<ProcessAudio>, PpdcError> {
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    // Log that the request has been received
    println!("Received a request to process audio via multipart/form-data");

    let mut filename = None;
    let mut file_path = None;

    // Process multipart form data
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        PpdcError::new(400, ErrorType::ApiError, format!("Multipart error: {}", e))
    })? {
        let file_name = field.file_name().map(|s| s.to_string());

        if let Some(file_name) = file_name {
            // Validate filename to prevent path traversal attacks
            if Path::new(&file_name).components().count() > 1 {
                return Err(PpdcError::new(400, ErrorType::ApiError, "Invalid filename: path traversal not allowed".to_string()));
            }
            
            // Create a safer temporary file path
            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join(&file_name);
            
            let data = field.bytes().await.map_err(|e| {
                PpdcError::new(400, ErrorType::ApiError, format!("Failed to read field data: {}", e))
            })?;
            let mut file = File::create(&path).await.map_err(|e| {
                PpdcError::new(500, ErrorType::InternalError, format!("Failed to create file: {}", e))
            })?;
            file.write_all(&data).await.map_err(|e| {
                PpdcError::new(500, ErrorType::InternalError, format!("Failed to write file: {}", e))
            })?;
            filename = Some(file_name);
            file_path = Some(path);
        }
    }

    // Transcribe the audio file using OpenAI API
    let transcription = if let Some(path) = file_path {
        match transcribe_audio_with_openai(&path).await {
            Ok(text) => Some(text),
            Err(e) => {
                println!("OpenAI transcription error: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Generate resource information using GPT API if transcription is available
    let resource_info = if let Some(transcript) = &transcription {
        match generate_resource_info_with_gpt(transcript).await {
            Ok(info) => Some(info),
            Err(e) => {
                println!("GPT resource info generation error: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Create new resource and interaction in the database if resource info is available
    let created_interaction = if let Some(info) = &resource_info {
        match create_resource_and_interaction(&pool, &session, info).await {
            Ok(interaction_with_resource) => Some(interaction_with_resource),
            Err(e) => {
                println!("Failed to create resource and interaction: {}", e);
                None
            }
        }
    } else {
        None
    };

    let process_audio = ProcessAudio {
        message: "Audio file received, transcribed, and resource information extracted".to_string(),
        filename,
        transcription,
        resource_info,
        created_interaction,
        error: None,
    };

    Ok(Json(process_audio))
}

async fn transcribe_audio_with_openai(file_path: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    // Get OpenAI API key and base URL from environment
    let api_key = environment::get_openai_api_key();
    let base_url = environment::get_openai_api_base_url();
    
    let client = Client::new();
    
    // Read the file content into bytes
    let file_content = tokio::fs::read(file_path).await
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    // Get the filename for the multipart form
    let filename = file_path.file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid filename")?;
    
    // Create form data for file upload
    let form = reqwest::multipart::Form::new()
        .text("model", "whisper-1")
        .text("response_format", "text")
        .part("file", reqwest::multipart::Part::bytes(file_content)
            .file_name(filename.to_string()));
    
    // Make request to OpenAI API
    let response = client
        .post(&format!("{}/v1/audio/transcriptions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to OpenAI: {}", e))?;
    
    // Extract status code before consuming the response
    let status = response.status();
    
    if !status.is_success() {
        let error_text = response.text().await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("OpenAI API error ({}): {}", status, error_text).into());
    }
    
    // Parse response
    let transcription = response.text().await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    Ok(transcription)
}

async fn generate_resource_info_with_gpt(transcription: &str) -> Result<ResourceInfo, Box<dyn std::error::Error>> {
    // Get OpenAI API key and base URL from environment
    let api_key = environment::get_openai_api_key();
    let base_url = environment::get_openai_api_base_url();
    
    let client = Client::new();
    
    // Create the prompt for GPT to extract resource information in French
    let prompt = format!(
        "Basé sur la transcription audio suivante, extrait les informations sur tout livre, article de recherche ou ressource académique mentionné. \
         Si une ressource est mentionnée, extrait le titre et l'auteur (en considérant qu'ils peuvent être mal orthographiés), puis crée un résumé détaillé. \
         Si aucune ressource spécifique n'est mentionnée, fournis un résumé général du contenu audio.\n\n\
         Transcription: {}\n\n\
         Extrais les informations suivantes:\n\
         - title: Le titre de la ressource (ou 'Contenu Général' si aucune ressource spécifique)\n\
         - author: L'auteur(s) de la ressource (ou 'N/A' si aucune ressource spécifique)\n\
         - project: Le projet ou contexte dans lequel cette ressource est mentionnée\n\
         - interaction_comment: A partir du projet indiqué dans la transcription, développe pourquoi cette ressource est importante pour le projet (ou 'N/A' si aucune ressource spécifique)\n\
         - resource_summary: Un résumé détaillé (200-300 mots) en français, se concentrant sur les idées principales, les découvertes clés et la signification\n\
         - estimated_publishing_date: Estime la date de publication de cette ressource au format YYYY-MM-DD (ou 'N/A' si aucune ressource spécifique ou si la date ne peut pas être estimée)",
        transcription
    );
    
    // Create the GPT request with JSON schema response format
    let gpt_request = GPTRequest {
        model: "gpt-4o-mini".to_string(),
        messages: vec![
            GPTMessage {
                role: "system".to_string(),
                content: "Tu es un expert en extraction et résumé de ressources académiques et littéraires. Réponds toujours avec du JSON valide selon le schéma fourni.".to_string(),
            },
            GPTMessage {
                role: "user".to_string(),
                content: prompt,
            },
        ],
        max_tokens: 800,
        temperature: 0.3, // Lower temperature for more consistent extraction
        response_format: GPTResponseFormat {
            response_type: "json_schema".to_string(),
            json_schema: JsonSchema {
                name: "reference_schema".to_string(),
                schema: SchemaDefinition {
                    schema_type: "object".to_string(),
                    properties: Properties {
                        title: PropertyDefinition {
                            property_type: "string".to_string(),
                        },
                        author: PropertyDefinition {
                            property_type: "string".to_string(),
                        },
                        project: PropertyDefinition {
                            property_type: "string".to_string(),
                        },
                        interaction_comment: PropertyDefinition {
                            property_type: "string".to_string(),
                        },
                        resource_summary: PropertyDefinition {
                            property_type: "string".to_string(),
                        },
                        estimated_publishing_date: PropertyDefinition {
                            property_type: "string".to_string(),
                        },
                    },
                    required: vec!["title".to_string(), "author".to_string()],
                },
            },
        },
    };
    
    // Make request to GPT API
    let response = client
        .post(&format!("{}/v1/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&gpt_request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to GPT: {}", e))?;
    
    // Extract status code before consuming the response
    let status = response.status();
    
    if !status.is_success() {
        let error_text = response.text().await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("GPT API error ({}): {}", status, error_text).into());
    }
    
    // Parse response
    let gpt_response: GPTResponse = response.json().await
        .map_err(|e| format!("Failed to parse GPT response: {}", e))?;
    
    // Extract the JSON content from the response
    let json_content = gpt_response.choices
        .first()
        .and_then(|choice| Some(choice.message.content.clone()))
        .ok_or("No response content from GPT")?;
    
    // Parse the JSON response into ResourceInfo
    let resource_info: ResourceInfo = serde_json::from_str(&json_content)
        .map_err(|e| format!("Failed to parse GPT JSON response: {}", e))?;
    
    Ok(resource_info)
}

async fn create_resource_and_interaction(
    pool: &DbPool, 
    session: &Session, 
    resource_info: &ResourceInfo
) -> Result<InteractionWithResource, Box<dyn std::error::Error>> {
    // Get user ID from session
    let user_id = session.user_id.ok_or("User not authenticated")?;
    
    // Determine resource type based on the content
    let resource_type = if resource_info.title != "Contenu Général" {
        ResourceType::Book // Default to Book, could be enhanced with better detection
    } else {
        ResourceType::Idea
    };
    
    // Create new resource
    let new_resource = NewResource {
        title: resource_info.title.clone(),
        subtitle: format!("Écrit par {} - sur le thème {}", resource_info.author.clone(), resource_info.project.clone()),
        content: Some(resource_info.resource_summary.clone()),
        external_content_url: None,
        comment: None,
        image_url: None,
        resource_type: Some(resource_type),
        maturing_state: Some(MaturingState::Finished),
        publishing_state: Some("pbsh".to_string()),
        category_id: None,
        is_external: Some(true),
    };
    
    // Save resource to database
    let created_resource = new_resource.create(pool)
        .map_err(|e| format!("Failed to create resource: {}", e))?;
    
    // Create input interaction (when user records the voice message)
    let new_input_interaction = NewInteraction {
        interaction_user_id: Some(user_id),
        interaction_progress: 0,
        interaction_comment: Some(format!("Ressource créée à partir d'un message vocal: {}", resource_info.interaction_comment)),
        interaction_date: Some(Utc::now().naive_utc()),
        interaction_type: Some("inpt".to_string()), // Input type
        interaction_is_public: Some(true),
        resource_id: Some(created_resource.id),
    };
    
    // Save input interaction to database
    let created_input_interaction = save_interaction(pool, &new_input_interaction)
        .map_err(|e| format!("Failed to create input interaction: {}", e))?;
    
    // Create output interaction (representing the resource itself)
    let output_interaction_date = if resource_info.estimated_publishing_date != "N/A" {
        // Try to parse the estimated publishing date
        match chrono::NaiveDate::parse_from_str(&resource_info.estimated_publishing_date, "%Y-%m-%d") {
            Ok(date) => date.and_hms_opt(12, 0, 0).unwrap_or_else(|| Utc::now().naive_utc()),
            Err(_) => Utc::now().naive_utc(), // Fallback to current time if parsing fails
        }
    } else {
        Utc::now().naive_utc() // Use current time if no date provided
    };
    
    let new_output_interaction = NewInteraction {
        interaction_user_id: Some(user_id),
        interaction_progress: 100, // Resource is complete/finished
        interaction_comment: Some(format!("Ressource créée: {}", resource_info.title)),
        interaction_date: Some(output_interaction_date),
        interaction_type: Some("outp".to_string()), // Output type
        interaction_is_public: Some(true),
        resource_id: Some(created_resource.id),
    };
    
    // Save output interaction to database
    let created_output_interaction = save_interaction(pool, &new_output_interaction)
        .map_err(|e| format!("Failed to create output interaction: {}", e))?;
    
    // Create InteractionWithResource for response (using the output interaction)
    let interaction_with_resource = InteractionWithResource {
        interaction: created_output_interaction,
        resource: created_resource,
    };
    
    Ok(interaction_with_resource)
}

fn save_interaction(pool: &DbPool, new_interaction: &NewInteraction) -> Result<Interaction, Box<dyn std::error::Error>> {
    use diesel::insert_into;
    use crate::schema::interactions;
    
    let mut conn = pool.get().expect("Failed to get a connection from the pool");
    
    let result = insert_into(interactions::table)
        .values(new_interaction)
        .get_result(&mut conn)
        .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(result)
}

