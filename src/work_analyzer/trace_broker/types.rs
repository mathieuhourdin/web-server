use serde::{Deserialize, Serialize};
use crate::entities::resource::resource_type::ResourceType;

#[derive(Debug, Serialize, Deserialize)]
pub struct NewLandmarkForExtractedElement {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub identified: bool,
    pub landmark_type: ResourceType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchedExtractedElementForLandmark {
    pub title: String,
    pub subtitle: String,
    pub extracted_content: String,
    pub generated_context: String,
    pub landmark_id: Option<String>,
    pub landmark_type: ResourceType,
}