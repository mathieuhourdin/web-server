use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    resource::{resource_type::ResourceType, Resource},
    landmark::Landmark,
};
use crate::work_analyzer::trace_broker::broke_for_resource::{ResourceProcessor, split_trace_in_elements_for_resources_landmarks};
use crate::work_analyzer::trace_broker::traits::ProcessorContext;
use chrono::NaiveDate;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod broke_for_resource;
pub mod trace_qualify;
pub mod types;
pub mod traits;

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
    let resource_processor = ResourceProcessor::new();
    let context = ProcessorContext {
        landmarks: landmarks.into_iter().map(|landmark| Landmark::from_resource((*landmark).clone())).collect::<Vec<Landmark>>(),
        trace: trace.clone(),
        user_id: user_id,
        analysis_resource_id: analysis_resource_id,
        pool: pool.clone(),
    };
    let new_landmarks = resource_processor.process(&context).await?;
    let new_resources = new_landmarks.into_iter().map(|landmark| Landmark::to_resource(landmark.clone())).collect::<Vec<Resource>>();
    Ok(new_resources)
}