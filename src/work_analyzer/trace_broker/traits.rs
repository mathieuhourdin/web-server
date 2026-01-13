use crate::entities::{
    resource::{Resource, resource_type::ResourceType},
    error::PpdcError,
    landmark::Landmark,
};
use crate::openai_handler::GptRequestConfig;
use uuid::Uuid;
use crate::db::DbPool;
use async_trait::async_trait;

pub trait NewLandmarkForExtractedElement {
    fn title(&self) -> String;
    fn subtitle(&self) -> String;
    fn content(&self) -> String;
    fn identified(&self) -> bool;
    fn landmark_type(&self) -> ResourceType;
}

pub trait MatchedExtractedElementForLandmark {
    fn title(&self) -> String;
    fn subtitle(&self) -> String;
    fn extracted_content(&self) -> String;
    fn generated_context(&self) -> String;
    fn landmark_id(&self) -> Option<String>;
    fn landmark_type(&self) -> ResourceType;
}

pub trait ExtractedElementForLandmark {
    fn reference(&self) -> String;
    fn extracted_content(&self) -> String;
    fn generated_context(&self) -> String;
}

// I create a trait. 
// Each processor will implement this trait.
// For each landmark type, a processor object is created that processes the trace for this landmark type.
// But shouldn't I create a global processor with the state of the global processing, and maybe a state ?
// Should be called a pipline maybe ?

// Processor context is created at runtime with the data related to the current processing.
pub struct ProcessorContext {
    pub landmarks: Vec<Landmark>,
    pub trace: Resource,
    pub user_id: Uuid,
    pub analysis_resource_id: Uuid,
    pub pool: DbPool,
}

// Processor config is created at startup with the config for each processor type.
// Processing has three steps : 
// 1. Extract elements from the trace.
// 2. Match elements to landmarks.
// 3. Create new landmarks if needed.
// Then we need to create three prompts for each step.
// I think we need a struct for gpt config.
pub struct ProcessorConfig {
    pub landmark_type: ResourceType,
    pub extract_elements_gpt_config: GptRequestConfig,
    pub match_elements_gpt_config: GptRequestConfig,
    pub create_new_landmarks_gpt_config: GptRequestConfig,
}

#[async_trait]
pub trait LandmarkProcessor: Send + Sync {
    
    type ExtractedElement: ExtractedElementForLandmark + Send;
    type MatchedElement: MatchedExtractedElementForLandmark + Send;
    type NewLandmark: NewLandmarkForExtractedElement + Send;

    async fn extract_elements(
        &self,
        existing_landmarks: &Vec<Landmark>,
        context: &ProcessorContext,
    ) -> Result<Vec<Self::ExtractedElement>, PpdcError>;

    async fn match_elements(
        &self,
        existing_landmarks: &Vec<Landmark>,
        context: &ProcessorContext,
    ) -> Result<Vec<Self::MatchedElement>, PpdcError>;

    async fn create_new_landmarks(
        &self,
        matched_elements: &Vec<Self::MatchedElement>,
        context: &ProcessorContext,
    ) -> Result<Vec<Self::NewLandmark>, PpdcError>;

    async fn process(
        &self,
        existing_landmarks: &Vec<Landmark>,
        context: &ProcessorContext,
    ) -> Result<Vec<Self::NewLandmark>, PpdcError>;
}
