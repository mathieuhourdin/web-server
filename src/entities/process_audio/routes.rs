

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
use crate::entities::process_audio::gpt_handler::{generate_resource_info_with_gpt, select_problems_with_gpt};
use crate::entities::process_audio::whisper_handler::transcribe_audio_with_openai;
use crate::entities::process_audio::database::{create_resource_and_interaction, find_or_create_user_by_author_name};
use crate::entities::process_audio::gpt_handler::SelectedProblems;
use crate::entities::resource_relation::NewResourceRelation;


#[derive(Serialize)]
pub struct ProcessAudio {
    pub message: String,
    pub filename: Option<String>,
    pub transcription: Option<String>,
    pub resource_info: Option<ResourceInfo>,
    pub created_interaction: InteractionWithResource,
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
            Ok(interaction_with_resource) => interaction_with_resource,
            Err(e) => {
                println!("Failed to create resource and interaction: {}", e);
                return Err(PpdcError::new(500, ErrorType::InternalError, format!("Failed to create resource and interaction: {}", e)))
            }
        }
    } else {
        return Err(PpdcError::new(500, ErrorType::InternalError, "Resource info is required".to_string()))
    };

    let user_problems_resources: Vec<InteractionWithResource> = Interaction::find_paginated_outputs_problems(0, 100, session.user_id.unwrap(), &pool)?;

    let selected_problems = select_problems_with_gpt(
        &created_interaction.resource.title,
        "auteur",
        &created_interaction.resource.content, 
        created_interaction.interaction.interaction_comment.as_deref().unwrap_or(""), 
        &user_problems_resources
    ).await;

    let selected_problems = match selected_problems {
        Ok(selected_problems) => selected_problems,
        Err(e) => return Err(PpdcError::new(500, ErrorType::InternalError, format!("Error selecting problems: {}", e)))
    };

    for problem in selected_problems {
        let problem_uuid = uuid::Uuid::parse_str(&problem.problem_id)
            .map_err(|e| PpdcError::new(400, ErrorType::InternalError, format!("Invalid problem ID format: {}", e)))?;
        
        let new_resource_relation = NewResourceRelation {
            origin_resource_id: created_interaction.resource.id.clone(),
            target_resource_id: problem_uuid,
            user_id: Some(session.user_id.unwrap()),
            relation_type: Some("bibl".to_string()),
            relation_comment: problem.relation_comment.clone(),
        };
        new_resource_relation.create(&pool)?;
    }



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







