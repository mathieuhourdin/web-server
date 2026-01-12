use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::NewInteraction,
    resource::{resource_type::ResourceType, NewResource, Resource},
    resource_relation::NewResourceRelation,
};
use crate::work_analyzer::trace_broker::broke_for_resource::split_trace_in_elements_for_resources_landmarks;
use crate::openai_handler::gpt_responses_handler::make_gpt_request;
use crate::work_analyzer::context_builder::{get_ontology_context, get_user_biography};
use chrono::NaiveDate;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod broke_for_resource;
pub mod trace_qualify;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceExtractionProperties {
    pub title: String,
    pub subtitle: String,
    pub interaction_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleElement {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub enriched_content: String,
    pub element_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedElementForResource {
    pub resource: String,
    pub extracted_content: String,
    pub generated_context: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleElementWithResource {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub enriched_content: String,
    pub element_type: String,
    pub resource_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleElements {
    pub elements: Vec<SimpleElement>,
}

pub async fn process_trace(
    trace: &Resource,
    user_id: Uuid,
    landmarks: &Vec<Resource>,
    analysis_resource_id: Uuid,
    pool: &DbPool,
) -> Result<Vec<Resource>, PpdcError> {
    let resources_landmarks = landmarks.into_iter().filter(|landmark| landmark.resource_type == ResourceType::Resource).collect::<Vec<&Resource>>();
    let elements = split_trace_in_elements_for_resources_landmarks(user_id, analysis_resource_id, trace, &resources_landmarks, pool).await?;
    Ok(elements)
}