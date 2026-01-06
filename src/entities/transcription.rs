use axum::{
    extract::{Multipart},
    Json,
};
use serde::Serialize;
use crate::entities::error::{PpdcError, ErrorType};
use crate::openai_handler::whisper_handler::transcribe_audio_with_openai;
use std::path::Path;

#[derive(Serialize)]
pub struct Transcription {
    pub content: String,
}

// This route receives audio files via multipart/form-data and transcribes them using OpenAI
pub async fn post_transcription_route(
    mut multipart: Multipart,
) -> Result<Json<Transcription>, PpdcError> {
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    // Log that the request has been received
    println!("Received a request to process audio via multipart/form-data");

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
    Ok(Json(Transcription { content: transcription.unwrap_or("".to_string()) }))
}







