use crate::environment;
use reqwest::Client;
use serde::{Serialize, Deserialize};


pub async fn transcribe_audio_with_openai(file_path: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
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