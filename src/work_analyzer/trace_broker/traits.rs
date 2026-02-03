use crate::entities::{
    error::PpdcError, 
    resource::{
        resource_type::ResourceType,
    }, 
};
use std::fmt::Debug;
use crate::entities_v2::{
    landmark::{
        Landmark,
        NewLandmark,
        landmark_create_copy_child_and_return,
    },
    trace::Trace,
    trace_mirror::TraceMirror,
    element::{
        NewElement,
        ElementType,
    },
    landscape_analysis::{
        self,
    },
};
use crate::openai_handler::GptRequestConfig;
use uuid::Uuid;
use crate::db::DbPool;
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use crate::work_analyzer::matching::{
    ElementWithIdentifier,
    self
};
use crate::work_analyzer::trace_broker::types::IdentityState;
use std::collections::HashMap;

pub trait NewLandmarkForExtractedElement {
    fn title(&self) -> String;
    fn subtitle(&self) -> String;
    fn content(&self) -> String;
    fn identity_state(&self) -> IdentityState;
    fn landmark_type(&self) -> ResourceType;

    fn to_new_landmark(
        &self,
        analysis_id: Uuid,
        user_id: Uuid,
        parent_id: Option<Uuid>,
    ) -> NewLandmark {
        NewLandmark::new(
            self.title(),
            self.subtitle(),
            self.content(),
            self.landmark_type(),
            self.identity_state().into(),
            analysis_id,
            user_id,
            parent_id,
        )
    }
}

pub trait MatchedExtractedElementForLandmark {
    fn reference(&self) -> String;
    fn description(&self) -> String;
    fn extracted_content(&self) -> String;
    fn generated_context(&self) -> String;
    fn landmark_id(&self) -> Option<String>;
    fn confidence(&self) -> f32;

    fn to_new_element(
        &self,
        trace_id: Uuid,
        trace_mirror_id: Option<Uuid>,
        landmark_id: Option<Uuid>,
        analysis_id: Uuid,
        user_id: Uuid,
    ) -> NewElement {
        NewElement::new(
            self.reference(),
            self.description(),
            format!("{} - {}", self.extracted_content(), self.generated_context()),
            ElementType::Event,
            trace_id,
            trace_mirror_id,
            landmark_id,
            analysis_id,
            user_id,
        )
    }
    fn new(
        reference: String,
        extracted_content: String,
        generated_context: String,
        landmark_id: Option<String>,
        confidence: f32,
    ) -> Self
    where Self: Sized;
}

pub trait ExtractedElementForLandmark {
    fn reference(&self) -> String;
    fn extracted_content(&self) -> String;
    fn generated_context(&self) -> String;
}

impl<T: ExtractedElementForLandmark> ElementWithIdentifier for T {
    fn identifier(&self) -> String {
        self.reference()
    }
}

// I create a trait. 
// Each processor will implement this trait.
// For each landmark type, a processor object is created that processes the trace for this landmark type.
// But shouldn't I create a global processor with the state of the global processing, and maybe a state ?
// Should be called a pipline maybe ?

// Processor context is created at runtime with the data related to the current processing.
pub struct ProcessorContext {
    pub landmarks: Vec<Landmark>,
    pub trace: Trace,
    pub trace_mirror: TraceMirror,
    pub user_id: Uuid,
    pub landscape_analysis_id: Uuid,
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
    
    type ExtractedElement: ExtractedElementForLandmark + Send + Serialize + DeserializeOwned + Debug + Clone;
    type MatchedElement: MatchedExtractedElementForLandmark + Send + From<(Self::ExtractedElement, Option<String>, f32)> + Clone;
    type NewLandmark: NewLandmarkForExtractedElement + Send;

    fn new() -> Self;

    fn extract_system_prompt(&self) -> String;
    fn extract_schema(&self) -> serde_json::Value;
    fn create_new_landmark_system_prompt(&self) -> String;
    fn create_new_landmark_schema(&self) -> serde_json::Value;

    async fn extract_elements(
        &self,
        context: &ProcessorContext,
    ) -> Result<Vec<Self::ExtractedElement>, PpdcError>;

    async fn match_elements(
        &self,
        elements: Vec<Self::ExtractedElement>,
        context: &ProcessorContext,
    ) -> Result<Vec<Self::MatchedElement>, PpdcError>;

    async fn match_elements_base(
        &self,
        elements: Vec<Self::ExtractedElement>,
        landmarks: &Vec<Landmark>,
    ) -> Result<Vec<Self::MatchedElement>, PpdcError> {

        let matched_elements = matching::match_elements::<Self::ExtractedElement>(elements, landmarks, None).await?;
        let matched_elements: Vec<Self::MatchedElement> = matched_elements
            .iter()
            .map(|element| {
                Self::MatchedElement::new(
                    element.element.identifier(),
                    element.element.extracted_content(),
                    element.element.generated_context(),
                    element.candidate_id.clone(),
                    element.confidence,
                )
            })
            .collect();
        return Ok(matched_elements);
    }

    async fn create_new_landmarks_and_elements(
        &self,
        matched_elements: Vec<Self::MatchedElement>,
        context: &ProcessorContext,
    ) -> Result<Vec<Landmark>, PpdcError> {
        println!("work_analyzer::trace_broker::LandmarkProcessor::create_new_landmarks_and_elements");
        let mut new_landmarks: Vec<Landmark> = vec![];
        let mut updated_landmark_ids: Vec<Uuid> = vec![];
        let mut landmarks_updates: HashMap<Uuid, Uuid> = HashMap::new();
        for element in matched_elements {
            let created_landmark;
            if element.landmark_id().is_none() {
                let new_resource_proposition = self.get_new_landmark_for_extracted_element(&element, context).await?;
                let new_landmark = new_resource_proposition.to_new_landmark(context.landscape_analysis_id, context.user_id, None);
                created_landmark = new_landmark.create(&context.pool)?;
            } else {
                // If two elements match the same landmark, the two should be linked to the same new landmark.
                let updated_landmark_id = Uuid::parse_str(element.landmark_id().clone().unwrap().as_str())?;
                if landmarks_updates.contains_key(&updated_landmark_id) {
                    created_landmark = Landmark::find(landmarks_updates.get(&updated_landmark_id).unwrap().clone(), &context.pool)?;
                } else {
                    // If it is the first reference to this landmark, we create a new landmark and link it to the parent landmark.
                    created_landmark = landmark_create_copy_child_and_return(updated_landmark_id, context.user_id, context.landscape_analysis_id, &context.pool)?;
                    landmarks_updates.insert(updated_landmark_id, created_landmark.id);
                    updated_landmark_ids.push(updated_landmark_id);
                }
            }
            //let element = MatchedExtractedElementForLandmarkType::from(element);
            let _ = element.to_new_element(
                context.trace.id, 
                Some(context.trace_mirror.id),
                Some(created_landmark.id), 
                context.landscape_analysis_id, 
                context.user_id
            )
            .create(&context.pool)?;
            new_landmarks.push(created_landmark);
        }
        for landmark in &context.landmarks {
            if !updated_landmark_ids.contains(&landmark.id) {
                landscape_analysis::add_landmark_ref(context.landscape_analysis_id, landmark.id, context.user_id, &context.pool)?;
            }
        }
        Ok(new_landmarks)
    }

    async fn get_new_landmark_for_extracted_element(
        &self,
        elements: &Self::MatchedElement,
        context: &ProcessorContext,
    ) -> Result<Self::NewLandmark, PpdcError>;

    async fn process(
        &self,
        context: &ProcessorContext,
    ) -> Result<Vec<Landmark>, PpdcError>;
}
