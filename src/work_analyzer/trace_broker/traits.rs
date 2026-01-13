use crate::entities::resource::{Resource, resource_type::ResourceType};
use crate::openai_handler::GptRequestConfig;
use uuid::Uuid;
use crate::db::DbPool;

// I create a trait. 
// Each processor will implement this trait.
// For each landmark type, a processor object is created that processes the trace for this landmark type.
// But shouldn't I create a global processor with the state of the global processing, and maybe a state ?
// Should be called a pipline maybe ?

// Processor context is created at runtime with the data related to the current processing.
pub struct ProcessorContext {
    pub landmarks: Vec<Resource>,
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

pub trait LandmarkProcessor: Send + Sync {
}
