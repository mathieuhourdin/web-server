use serde::{Deserialize, Serialize};
use crate::entities::resource::resource_type::ResourceType;
use crate::work_analyzer::trace_broker::traits::NewLandmarkForExtractedElement;


#[derive(Debug, Serialize, Deserialize)]
pub struct NewLandmarkForExtractedElementType {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub identified: bool,
    pub landmark_type: ResourceType,
}

impl NewLandmarkForExtractedElement for NewLandmarkForExtractedElementType {
    fn title(&self) -> String {
        self.title.clone()
    }
    fn subtitle(&self) -> String {
        self.subtitle.clone()
    }
    fn content(&self) -> String {
        self.content.clone()
    }
    fn identified(&self) -> bool {
        self.identified
    }
    fn landmark_type(&self) -> ResourceType {
        self.landmark_type
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchedExtractedElementForLandmarkType {
    pub title: String,
    pub subtitle: String,
    pub extracted_content: String,
    pub generated_context: String,
    pub landmark_id: Option<String>,
    pub landmark_type: ResourceType,
}