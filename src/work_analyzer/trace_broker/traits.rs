use crate::{entities::{
    error::PpdcError, resource::{NewResource, Resource, resource_type::ResourceType}, resource_relation::NewResourceRelation
}};
use std::fmt::Debug;
use crate::entities_v2::{
    landmark::{
        Landmark,
        NewLandmark,
        landmark_create_copy_child_and_return,
    },
    trace::Trace,
};
use crate::openai_handler::GptRequestConfig;
use uuid::Uuid;
use crate::db::DbPool;
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use crate::work_analyzer::trace_broker::matching::{
    LocalArray,
    Matches,
    LandmarkForMatching,
    self
};
use crate::work_analyzer::trace_broker::types::IdentityState;
use std::collections::HashMap;

pub struct ElementLandmarkMatchingResult {
    pub element: Option<String>,
    pub landmark_id: Option<String>,
    pub confidence: f32,
}

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

    fn to_new_resource(&self) -> NewResource {
        NewResource::new(
            self.reference(),
            self.description(),
            format!("{} - {}", self.extracted_content(), self.generated_context()),
            ResourceType::Event
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

// I create a trait. 
// Each processor will implement this trait.
// For each landmark type, a processor object is created that processes the trace for this landmark type.
// But shouldn't I create a global processor with the state of the global processing, and maybe a state ?
// Should be called a pipline maybe ?

// Processor context is created at runtime with the data related to the current processing.
pub struct ProcessorContext {
    pub landmarks: Vec<Landmark>,
    pub trace: Trace,
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
    
    type ExtractedElement: ExtractedElementForLandmark + Send + Serialize + DeserializeOwned + Debug;
    type MatchedElement: MatchedExtractedElementForLandmark + Send + From<(Self::ExtractedElement, Option<String>, f32)>;
    type NewLandmark: NewLandmarkForExtractedElement + Send;

    fn new() -> Self;

    fn extract_system_prompt(&self) -> String;
    fn extract_schema(&self) -> serde_json::Value;
    fn create_new_landmark_system_prompt(&self) -> String;
    fn create_new_landmark_schema(&self) -> serde_json::Value;
    fn match_elements_system_prompt(&self) -> String;
    fn match_elements_schema(&self) -> serde_json::Value;

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

        let mut matched_elements: Vec<Self::MatchedElement> = vec![];
        let mut elements = elements;
        
        // Collect references of matched elements to remove them later
        let mut matched_references: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        for element in &elements {
            for landmark in landmarks {
                if element.reference() == landmark.title {
                    matched_elements.push(Self::MatchedElement::new(
                        element.reference(),
                        element.extracted_content(),
                        element.generated_context(),
                        Some(landmark.id.to_string()),
                        1.0,
                    ));
                    matched_references.insert(element.reference());
                    break; // Found a match, no need to check other landmarks
                }
            }
        }
        
        // Remove matched elements from the vector
        elements.retain(|element| !matched_references.contains(&element.reference()));

        let landmarks_for_matching: Vec<LandmarkForMatching> = landmarks.iter().map(|landmark| LandmarkForMatching::from(landmark)).collect();
        let elements_local_array = LocalArray::from_vec(elements);
        let landmarks_local_array = LocalArray::from_vec(landmarks_for_matching);
        let user_prompt = format!("
            Elements: {}\n\n
            Candidates: {}\n\n
        ",
        serde_json::to_string(&elements_local_array.items)?,
        serde_json::to_string(&landmarks_local_array.items)?);
        let gpt_request_config = GptRequestConfig::new(
            "gpt-4.1-nano".to_string(),
            &self.match_elements_system_prompt(),
            &user_prompt,
            Some(self.match_elements_schema())
        );
        let matching_results: Matches = gpt_request_config.execute().await?;
        println!("work_analyzer::trace_broker::LandmarkProcessor::match_elements_base matching_results: {:?}", matching_results);
        println!("work_analyzer::trace_broker::LandmarkProcessor::match_elements_base elements_local_array: {:?}", elements_local_array);
        println!("work_analyzer::trace_broker::LandmarkProcessor::match_elements_base landmarks_local_array: {:?}", landmarks_local_array);
        let attached_elements = matching::attach_matching_results_to_elements(matching_results.matches, elements_local_array, landmarks_local_array);

        matched_elements.extend(attached_elements);

        Ok(matched_elements)
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
                let new_landmark = new_resource_proposition.to_new_landmark(context.analysis_resource_id, context.user_id, None);
                created_landmark = new_landmark.create(&context.pool)?;
            } else {
                // If two elements match the same landmark, the two should be linked to the same new landmark.
                let updated_landmark_id = Uuid::parse_str(element.landmark_id().clone().unwrap().as_str())?;
                if landmarks_updates.contains_key(&updated_landmark_id) {
                    created_landmark = Landmark::find(landmarks_updates.get(&updated_landmark_id).unwrap().clone(), &context.pool)?;
                } else {
                    // If it is the first reference to this landmark, we create a new landmark and link it to the parent landmark.
                    created_landmark = landmark_create_copy_child_and_return(updated_landmark_id, context.user_id, context.analysis_resource_id, &context.pool)?;
                    landmarks_updates.insert(updated_landmark_id, created_landmark.id);
                    updated_landmark_ids.push(updated_landmark_id);
                }
            }
            //let element = MatchedExtractedElementForLandmarkType::from(element);
            let _ = Self::element_create_for_trace_landmark_and_analysis(element, context.trace.id, created_landmark.id, context.analysis_resource_id, context.user_id, &context.pool);
            new_landmarks.push(created_landmark);
        }
        for landmark in &context.landmarks {
            println!("work_analyzer::trace_broker::LandmarkProcessor::create_new_landmarks_and_elements landmark id : {}", landmark.id);
            println!("work_analyzer::trace_broker::LandmarkProcessor::create_new_landmarks_and_elements updated_landmark_ids : {:?}", updated_landmark_ids);
            if !updated_landmark_ids.contains(&landmark.id) {
                println!("work_analyzer::trace_broker::LandmarkProcessor::create_new_landmarks_and_elements creating new resource relation for landmark id : {}", landmark.id);
                let mut new_resource_relation = NewResourceRelation::new(landmark.id, context.analysis_resource_id);
                new_resource_relation.relation_type = Some("refr".to_string());
                new_resource_relation.user_id = Some(context.user_id);
                new_resource_relation.create(&context.pool)?;
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


    fn element_create_for_trace_landmark_and_analysis(
        element: Self::MatchedElement,
        trace_id: Uuid,
        landmark_id: Uuid,
        analysis_resource_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Resource, PpdcError> {
        let new_element = element.to_new_resource();
        let created_element = new_element.create(pool)?;
        let mut new_resource_relation = NewResourceRelation::new(created_element.id, trace_id);
        new_resource_relation.relation_type = Some("elmt".to_string());
        new_resource_relation.user_id = Some(user_id);
        new_resource_relation.create(pool)?;
        let mut new_resource_relation = NewResourceRelation::new(created_element.id, landmark_id);
        new_resource_relation.relation_type = Some("elmt".to_string());
        new_resource_relation.user_id = Some(user_id);
        new_resource_relation.create(pool)?;
        let mut new_analysis_relation= NewResourceRelation::new(created_element.id, analysis_resource_id);
        new_analysis_relation.relation_type = Some("ownr".to_string());
        new_analysis_relation.user_id = Some(user_id);
        new_analysis_relation.create(pool)?;
        Ok(created_element)
    }
}
