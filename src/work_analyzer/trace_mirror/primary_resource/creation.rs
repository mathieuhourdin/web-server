use crate::work_analyzer::trace_mirror::primary_resource::matching::PrimaryResourceMatched;
use serde::{Deserialize, Serialize};
use crate::openai_handler::GptRequestConfig;
use crate::entities::error::{PpdcError, ErrorType};
use crate::entities_v2::landmark::{Landmark, LandmarkType, NewLandmark};
use crate::entities::resource::MaturingState;
use crate::db::DbPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryResourceCreated {
    pub title: String,
    pub theme: Option<String>,
    pub author: Option<String>,
    pub content: String,
    pub identity_state: String,
}

pub async fn run(element: PrimaryResourceMatched, analysis_id: Uuid, user_id: Uuid, pool: &DbPool, log_header: &str) -> Result<Landmark, PpdcError> {
    if element.candidate_id.is_some() {
        return Err(PpdcError::new(400, ErrorType::ApiError, "Primary resource already created".to_string()));
    }
    let system_prompt = include_str!("../prompts/primary_resource/creation/system.md").to_string();
    let schema = include_str!("../prompts/primary_resource/creation/schema.json").to_string();
    let gpt_request_config = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        &serde_json::to_string(&element)?,
        Some(serde_json::from_str(&schema).unwrap())
    ).with_log_header(log_header);
    let primary_resource_created = gpt_request_config.execute().await?;
    let new_landmark = create_primary_resource(primary_resource_created, analysis_id, user_id, pool).await?;
    Ok(new_landmark)
}

pub async fn create_primary_resource(primary_resource_created: PrimaryResourceCreated, analysis_id: Uuid, user_id: Uuid, pool: &DbPool) -> Result<Landmark, PpdcError> {
    let new_landmark = NewLandmark::new(
        primary_resource_created.title,
        format!("Par {} sur {}", primary_resource_created.author.unwrap_or("".to_string()), primary_resource_created.theme.unwrap_or("".to_string())),
        primary_resource_created.content,
        LandmarkType::Resource,
        MaturingState::Draft,
        analysis_id,
        user_id,
        None
    );
    let new_landmark = new_landmark.create(&pool)?;
    Ok(new_landmark)
}
