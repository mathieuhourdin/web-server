use crate::entities::{
    resource::{NewResource, Resource, resource_type::ResourceType, maturing_state::MaturingState},
    resource_relation::NewResourceRelation,
    error::PpdcError,
};
use crate::entities_v2::landmark::{
    Landmark,
    NewLandmark,
    landmark_create_copy_child_and_return,
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
            if self.identified() { MaturingState::Finished } else { MaturingState::Draft },
            analysis_id,
            user_id,
            parent_id,
        )
    }
}

pub trait MatchedExtractedElementForLandmark {
    fn title(&self) -> String;
    fn subtitle(&self) -> String;
    fn extracted_content(&self) -> String;
    fn generated_context(&self) -> String;
    fn landmark_id(&self) -> Option<String>;
    fn landmark_type(&self) -> ResourceType;

    fn to_new_resource(&self) -> NewResource {
        NewResource::new(
            self.title(),
            self.subtitle(),
            self.extracted_content(),
            ResourceType::Event
        )
    }
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
        elements: &Vec<Self::ExtractedElement>,
        context: &ProcessorContext,
    ) -> Result<Vec<Self::MatchedElement>, PpdcError>;

    async fn create_new_landmarks_and_elements(
        &self,
        matched_elements: Vec<Self::MatchedElement>,
        context: &ProcessorContext,
    ) -> Result<Vec<Landmark>, PpdcError> {
        println!("work_analyzer::trace_broker::LandmarkProcessor::create_new_landmarks_and_elements");
        let mut new_landmarks: Vec<Landmark> = vec![];
        let mut updated_landmark_ids: Vec<Uuid> = vec![];
        for element in matched_elements {
            let created_landmark;
            if element.landmark_id().is_none() {
                let new_resource_proposition = self.get_new_landmark_for_extracted_element(&element, context).await?;
                let new_landmark = new_resource_proposition.to_new_landmark(context.analysis_resource_id, context.user_id, None);
                created_landmark = new_landmark.create(&context.pool)?;
            } else {
                let updated_landmark_id = Uuid::parse_str(element.landmark_id().clone().unwrap().as_str())?;
                created_landmark = landmark_create_copy_child_and_return(updated_landmark_id, context.user_id, context.analysis_resource_id, &context.pool)?;
                updated_landmark_ids.push(updated_landmark_id);
            }
            //let element = MatchedExtractedElementForLandmarkType::from(element);
            let _ = Self::element_create_for_trace_landmark_and_analysis(element, context.trace.id, &created_landmark, context.analysis_resource_id, context.user_id, &context.pool);
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
        landmark: &Landmark,
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
        let mut new_resource_relation = NewResourceRelation::new(created_element.id, landmark.id);
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
