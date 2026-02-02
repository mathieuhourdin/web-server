use crate::entities::error::PpdcError;
use crate::entities::resource::resource_type::ResourceType;
use crate::entities::resource::maturing_state::MaturingState;
use crate::entities_v2::landmark::Landmark;
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::trace_broker::types::IdentityState;
use crate::work_analyzer::trace_broker::{
    traits::{
        ProcessorContext,
        LandmarkProcessor,
        ExtractedElementForLandmark, MatchedExtractedElementForLandmark, NewLandmarkForExtractedElement
    }
};
use crate::work_analyzer::matching::LandmarkForMatching;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtractedElementForResource {
    pub resource_identifier: String,
    pub author: String,
    pub theme: Option<String>,
    pub extracted_content: String,
    pub generated_context: String,
}

impl ExtractedElementForLandmark for ExtractedElementForResource {
    fn reference(&self) -> String {
        self.resource_identifier.clone()
    }
    fn extracted_content(&self) -> String {
        self.extracted_content.clone()
    }
    fn generated_context(&self) -> String {
        self.generated_context.clone()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MatchedExtractedElementForResource {
    pub id: Option<String>,
    pub resource_identifier: String,
    pub author: String,
    pub theme: Option<String>,
    pub extracted_content: String,
    pub generated_context: String,
    pub resource_id: Option<String>,
    #[serde(skip_serializing)]
    pub confidence: f32,
}

impl MatchedExtractedElementForLandmark for MatchedExtractedElementForResource {
    fn reference(&self) -> String {
        self.resource_identifier.clone()
    }
    fn description(&self) -> String {
        format!("Auteur : {} - Thème :{}", self.author, self.theme.clone().unwrap_or_default())
    }
    fn extracted_content(&self) -> String {
        self.extracted_content.clone()
    }
    fn generated_context(&self) -> String {
        self.generated_context.clone()
    }
    fn landmark_id(&self) -> Option<String> {
        self.resource_id.clone()
    }
    fn confidence(&self) -> f32 {
        self.confidence
    }
    fn new(
        reference: String,
        extracted_content: String,
        generated_context: String,
        landmark_id: Option<String>,
        confidence: f32,
    ) -> Self {
        Self { 
            id: None, 
            resource_identifier: reference, 
            author: "".to_string(),
            theme: None,
            extracted_content, 
            generated_context, 
            resource_id: landmark_id, 
            confidence 
        }
    }

}

impl From<(ExtractedElementForResource, Option<String>, f32)> for MatchedExtractedElementForResource {
    fn from((element, candidate_id, confidence): (ExtractedElementForResource, Option<String>, f32)) -> Self {
        Self {
            id: None,
            resource_identifier: element.reference(),
            author: element.author.clone(),
            theme: element.theme.clone(),
            extracted_content: element.extracted_content(),
            generated_context: element.generated_context(),
            resource_id: candidate_id,
            confidence
        }
    }
}


#[derive(Debug, Serialize, Deserialize)]
pub struct MatchedElementsForResources {
    pub matches: Vec<MatchedExtractedElementForResource>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedElementsForResources {
    pub elements: Vec<ExtractedElementForResource>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewResourceForExtractedElement {
    pub title: String,
    pub author: String,
    pub content: String,
    pub identity_state: IdentityState,
}

impl NewLandmarkForExtractedElement for NewResourceForExtractedElement {
    fn title(&self) -> String {
        self.title.clone()
    }
    fn subtitle(&self) -> String {
        format!("Par {}", self.author)
    }
    fn content(&self) -> String {
        self.content.clone()
    }
    fn identity_state(&self) -> IdentityState {
        self.identity_state
    }
    fn landmark_type(&self) -> ResourceType {
        ResourceType::Resource
    }
}

pub struct ResourceProcessor {
}

#[async_trait]
impl LandmarkProcessor for ResourceProcessor {
    type ExtractedElement = ExtractedElementForResource;
    type MatchedElement = MatchedExtractedElementForResource;
    type NewLandmark = NewResourceForExtractedElement;

    fn new() -> Self {
        Self {
        }
    }

    fn extract_system_prompt(&self) -> String {
        let system_prompt = include_str!("prompts/landmark_resource/extraction/system.md").to_string();
        system_prompt
    }

    fn extract_schema(&self) -> serde_json::Value {
        let schema = include_str!("prompts/landmark_resource/extraction/schema.json").to_string();
        serde_json::from_str(&schema).unwrap()
    }

    fn create_new_landmark_system_prompt(&self) -> String {
        let system_prompt = include_str!("prompts/landmark_resource/creation/system.md").to_string();
        system_prompt
    }
    fn create_new_landmark_schema(&self) -> serde_json::Value {
        let schema = include_str!("prompts/landmark_resource/creation/schema.json").to_string();
        serde_json::from_str(&schema).unwrap()
    }

    async fn extract_elements(
        &self,
        context: &ProcessorContext,
    ) -> Result<Vec<ExtractedElementForResource>, PpdcError> {

        println!("work_analyzer::trace_broker::split_trace_in_elements_gpt_request");
        let _known_resources = context.landmarks
            .iter()
            .filter(|resource| {
                resource.landmark_type == ResourceType::Resource
                && (resource.maturing_state == MaturingState::Finished || resource.maturing_state == MaturingState::Review || resource.maturing_state == MaturingState::Draft)
            })
            .map(|resource| LandmarkForMatching::from(resource))
            .collect::<Vec<LandmarkForMatching>>();
        let trace_string = format!("{}\n{}\n{}", context.trace_mirror.title, context.trace_mirror.subtitle, context.trace_mirror.content);
        let user_prompt = format!("
        trace_text : \n {}
        ",
        trace_string);
        let gpt_request_config = GptRequestConfig::new(
            "gpt-4.1-nano".to_string(),
            &self.extract_system_prompt(),
            &user_prompt,
            Some(self.extract_schema())
        );
        let elements: ExtractedElementsForResources = gpt_request_config.execute().await?;
        Ok(elements.elements)
    }

    async fn match_elements(
        &self,
        elements: Vec<ExtractedElementForResource>,
        context: &ProcessorContext,
    ) -> Result<Vec<MatchedExtractedElementForResource>, PpdcError> {
        let landmarks = context.landmarks.clone()
            .into_iter()
            .filter(|resource| {
                resource.landmark_type == ResourceType::Resource
                && (resource.maturing_state == MaturingState::Finished || resource.maturing_state == MaturingState::Review || resource.maturing_state == MaturingState::Draft)
            })
            .collect::<Vec<Landmark>>();
        let matched_elements = self.match_elements_base(elements, &landmarks).await?;
        println!("work_analyzer::trace_broker::match_elements matched_elements: {:?}", matched_elements);
        Ok(matched_elements)
    }
    async fn get_new_landmark_for_extracted_element(
        &self,
        element: &MatchedExtractedElementForResource,
        _context: &ProcessorContext,
    ) -> Result<Self::NewLandmark, PpdcError> {

        println!("work_analyzer::trace_broker::get_new_resource_for_extracted_element_from_gpt_request");

        let extracted_element_string = serde_json::to_string(&element)?;
        let user_prompt = format!("User:
        Élément simple : {}\n\n", extracted_element_string);
        let gpt_request_config = GptRequestConfig::new(
            "gpt-4.1-nano".to_string(),
            &self.create_new_landmark_system_prompt(),
            &user_prompt,
            Some(self.create_new_landmark_schema())
        );
        let new_resource_for_extracted_element: NewResourceForExtractedElement = gpt_request_config.execute().await?;
        Ok(new_resource_for_extracted_element)
    }

    async fn process(
        &self,
        context: &ProcessorContext,
    ) -> Result<Vec<Landmark>, PpdcError> {
        let elements = self.extract_elements(context).await?;
        let mut matched_elements: Vec<MatchedExtractedElementForResource> = vec![];
        if !elements.is_empty() {
            matched_elements = self.match_elements(elements, context).await?;
        }
        let matched_elements = matched_elements;
        println!("work_analyzer::trace_broker::process matched_elements: {:?}", matched_elements);
        let new_landmarks = self.create_new_landmarks_and_elements(matched_elements, context).await?;
        Ok(new_landmarks)
        
    }
}