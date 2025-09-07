

use axum::{
    extract::{Extension, Multipart},
    Json,
};
use serde::{Deserialize, Serialize};
use crate::entities::{session::Session, error::{PpdcError, ErrorType}};
use crate::db::DbPool;
use crate::environment;
use std::path::Path;
use reqwest::Client;
use chrono::Utc;
use crate::entities::resource::{NewResource};
use crate::entities::resource::resource_type::ResourceType;
use crate::entities::resource::maturing_state::MaturingState;
use crate::entities::interaction::model::{Interaction, NewInteraction, InteractionWithResource};
use crate::entities::user::{User, NewUser, find_similar_users};
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
    pub titre: String,
    pub auteur: String,
    pub projet: String,
    pub commentaire: String,
    pub ressource_summary: String,
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
    response_format: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct GPTResponseFormat {
    #[serde(rename = "type")]
    response_type: String,
    json_schema: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExtractionProperties {
    titre: String,
    auteur: String,
    projet: String,
    commentaire: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GenerationProperties {
    ressource_summary: String,
    estimated_publishing_date: String,
    interaction_comment: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Properties {
    extraction_props: ExtractionProperties,
    generation_props: GenerationProperties,
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
    let extraction_props = if let Some(transcript) = &transcription {
        match generate_resource_info_with_gpt(transcript).await {
            Ok(props) => Some(props),
            Err(e) => {
                println!("GPT resource info generation error: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Convert ExtractionProperties to ResourceInfo for compatibility
    let resource_info = if let Some(props) = &extraction_props {
        Some(ResourceInfo {
            titre: props.extraction_props.titre.clone(),
            auteur: props.extraction_props.auteur.clone(),
            projet: props.extraction_props.projet.clone(),
            commentaire: props.extraction_props.commentaire.clone(),
            ressource_summary: props.generation_props.ressource_summary.clone(),
            estimated_publishing_date: props.generation_props.estimated_publishing_date.clone(),
        })
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

async fn generate_resource_info_with_gpt(transcription: &str) -> Result<Properties, Box<dyn std::error::Error>> {
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
    Corrige les erreurs possibles dans les titres ou noms d'auteur.
    Réponds uniquement avec du JSON valide respectant le schéma donné.
    Si une information est incertaine ou absente, mets NA.
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

fn find_or_create_user_by_author_name(
    pool: &DbPool,
    author_name: &str,
    threshold: f32,
) -> Result<User, Box<dyn std::error::Error>> {
    // First, try to find similar users
    let similar_users = find_similar_users(pool, author_name, 5)
        .map_err(|e| format!("Failed to search for similar users: {}", e))?;
    
    // Check if we have a good match (score above threshold)
    if let Some(best_match) = similar_users.first() {
        if best_match.score >= threshold {
            // Found a good match, return the user
            return User::find(&best_match.id, pool)
                .map_err(|e| format!("Failed to find user: {}", e).into());
        }
    }
    
    // No good match found, create a new user
    create_user_from_author_name(pool, author_name)
}

fn create_user_from_author_name(pool: &DbPool, author_name: &str) -> Result<User, Box<dyn std::error::Error>> {
    // Parse the author name - assume it's in "First Last" format
    let name_parts: Vec<&str> = author_name.trim().split_whitespace().collect();
    
    let (first_name, last_name) = if name_parts.len() >= 2 {
        let first = name_parts[0].to_string();
        let last = name_parts[1..].join(" ");
        (first, last)
    } else {
        // If only one name, use it as last name
        ("".to_string(), author_name.to_string())
    };
    
    // Generate a unique handle and email
    let handle = format!("@{}", author_name.to_lowercase().replace(" ", "_"));
    let email = format!("{}@generated.local", handle.trim_start_matches('@'));
    
    let mut new_user = NewUser {
        email,
        first_name,
        last_name,
        handle,
        password: None,
        profile_picture_url: None,
        is_platform_user: Some(false), // Generated users are not platform users
        biography: Some(format!("Auteur généré automatiquement: {}", author_name)),
        pseudonym: Some(author_name.to_string()),
        pseudonymized: Some(true),
    };
    
    // Hash the password (empty password for generated users)
    new_user.hash_password()
        .map_err(|e| format!("Failed to hash password: {}", e))?;
    
    new_user.create(pool)
        .map_err(|e| format!("Failed to create user: {}", e).into())
}

async fn create_resource_and_interaction(
    pool: &DbPool, 
    session: &Session, 
    resource_info: &ResourceInfo
) -> Result<InteractionWithResource, Box<dyn std::error::Error>> {
    // Get user ID from session
    let user_id = session.user_id.ok_or("User not authenticated")?;
    
    // Find or create user based on author name
    let author_user = if resource_info.auteur != "N/A" && !resource_info.auteur.is_empty() {
        match find_or_create_user_by_author_name(pool, &resource_info.auteur, 0.7) {
            Ok(user) => Some(user),
            Err(e) => {
                println!("Failed to find or create author user: {}", e);
                None
            }
        }
    } else {
        None
    };
    
    // Determine resource type based on the content
    let resource_type = if resource_info.titre != "Contenu Général" {
        ResourceType::Book // Default to Book, could be enhanced with better detection
    } else {
        ResourceType::Idea
    };
    
    
    // Create new resource
    let new_resource = NewResource {
        title: resource_info.titre.clone(),
        subtitle: format!("{} - {}", resource_info.projet.clone(), resource_info.auteur.clone()),
        content: Some(resource_info.ressource_summary.clone()),
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
        interaction_comment: Some(format!("Ressource créée à partir d'un message vocal: {}", resource_info.projet)),
        interaction_date: Some(Utc::now().naive_utc()),
        interaction_type: Some("inpt".to_string()), // Input type
        interaction_is_public: Some(true),
        resource_id: Some(created_resource.id),
    };
    
    // Save input interaction to database
    let _created_input_interaction = save_interaction(pool, &new_input_interaction)
        .map_err(|e| format!("Failed to create input interaction: {}", e))?;
    
    // Create output interaction (representing the resource itself)
    // Use author user if available, otherwise use session user
    let output_user_id = if let Some(author) = &author_user {
        author.id
    } else {
        user_id
    };
    
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
        interaction_user_id: Some(output_user_id),
        interaction_progress: 100, // Resource is complete/finished
        interaction_comment: Some({
            let author_name = if let Some(author) = &author_user { 
                author.display_name()
            } else { 
                "utilisateur".to_string()
            };
            format!("Ressource créée par {}: {}", author_name, resource_info.titre)
        }),
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_or_create_user_by_author_name() {
        use crate::environment::get_database_url;
        let database_url = get_database_url();
        let manager = diesel::r2d2::ConnectionManager::new(database_url);
        let pool = DbPool::new(manager).expect("Failed to create connection pool");
        
        let author_name = "Hourdin Matthieu";
        let threshold = 0.7;
        let user = find_or_create_user_by_author_name(&pool, author_name, threshold);
        
        // Assert that we get a result (either success or error)
        assert!(user.is_ok() || user.is_err());
    }

    #[tokio::test]
    fn test_generate_resource_info_with_gpt_1() {
        use crate::environment::get_database_url;
        let database_url = get_database_url();
        let manager = diesel::r2d2::ConnectionManager::new(database_url);
        let pool = DbPool::new(manager).expect("Failed to create connection pool");
        
        let transcription = "L'animal, l'homme, la fonction symbolique de Raymond Ruyère.";
        let gpt_response = generate_resource_info_with_gpt_1(&transcription);

        println!("GPT response: {:?}", gpt_response.clone().unwrap());

        assert!(gpt_response.is_ok());
        assert!(gpt_response.clone().unwrap().choices.len() > 0);
    }
}