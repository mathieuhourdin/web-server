use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::entities_v2::landmark::{Landmark, LandmarkType};
use crate::entities_v2::reference::model::{Reference, ReferenceType};
use crate::openai_handler::GptRequestConfig;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReferencesExtractionResult {
    pub references: Vec<ReferencesExtractionResultItem>,
    pub tagged_text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReferencesExtractionResultItem {
    pub tag_id: i32,
    pub mention: String,
    pub landmark_id: Option<i32>,
    pub identification_status: IdentificationStatus,
    pub description: String,
    pub title_suggestion: String,
    pub landmark_type: LandmarkType,
    pub reference_type: ReferenceType,
    pub reference_variants: Vec<String>,
    pub context_tags: Vec<String>,
    pub same_object_tag_id: Option<i32>,
    pub confidence: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentificationStatus {
    Matched,
    New,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LandmarkReferenceContextItem {
    #[serde(skip_serializing)]
    pub uuid: Uuid,
    pub landmark_id: i32,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub existing_references: Vec<ReferenceContextItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReferenceContextItem {
    pub local_id: i32,
    pub mentions: Vec<String>,
    pub context_tags: Vec<String>,
}

pub async fn get_references_drafts(
    text: &str,
    context: &[LandmarkReferenceContextItem],
    analysis_id: Uuid,
) -> Result<ReferencesExtractionResult, PpdcError> {
    let system_prompt = include_str!("../prompts/references_extraction/system.md").to_string();
    let user_prompt = format!(
        "trace_text : {}\n\n
        landmarks_to_match : {}",
        text,
        serde_json::to_string(context)?
    );
    let schema = include_str!("../prompts/references_extraction/schema.json").to_string();
    let gpt_request_config = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(serde_json::from_str(&schema)?),
        Some(analysis_id),
    );
    let result: ReferencesExtractionResult = gpt_request_config.execute().await?;
    Ok(result)
}

pub fn build_context(
    landmarks: &[Landmark],
    pool: &DbPool,
) -> Result<Vec<LandmarkReferenceContextItem>, PpdcError> {
    let mut landmark_index = 0;
    let mut reference_index = 0;
    let mut context = Vec::new();
    for landmark in landmarks {
        let mut landmark_context = LandmarkReferenceContextItem {
            uuid: landmark.id,
            landmark_id: landmark_index,
            title: landmark.title.clone(),
            subtitle: landmark.subtitle.clone(),
            content: landmark.content.clone(),
            existing_references: Vec::new(),
        };
        let existing_references = Reference::find_for_landmark(landmark.id, pool)?;
        for reference in existing_references {
            let reference_context = ReferenceContextItem {
                local_id: reference_index,
                mentions: vec![reference.mention.clone()],
                context_tags: reference.context_tags.clone(),
            };
            landmark_context.existing_references.push(reference_context);
            reference_index += 1;
        }
        context.push(landmark_context);
        landmark_index += 1;
    }
    Ok(context)
}
