use crate::work_analyzer::matching;
use crate::entities_v2::landmark::Landmark;
use crate::entities::error::{PpdcError, ErrorType};
use crate::work_analyzer::mirror_pipeline::primary_resource::suggestion::PrimaryResourceSuggestion;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryResourceMatched {
    pub resource_identifier: String,
    pub theme: Option<String>,
    pub author: Option<String>,
    pub evidence: Vec<String>,
    pub candidate_id: Option<String>,
    pub confidence: f32,
}


pub async fn run(element: PrimaryResourceSuggestion, landmarks: &Vec<Landmark>, log_header: &str) -> Result<PrimaryResourceMatched, PpdcError> {
    let matched_elements = matching::match_elements::<PrimaryResourceSuggestion>(
        vec![element],
        landmarks,
        None,
        Some(log_header),
    ).await?;

    let matched_element = matched_elements
    .iter()
    .map(|element| PrimaryResourceMatched {
        resource_identifier: element.element.resource_identifier.clone(),
        theme: element.element.theme.clone(),
        author: element.element.author.clone(),
        evidence: element.element.evidence.clone(),
        candidate_id: element.candidate_id.clone(),
        confidence: element.confidence,
    })
    .max_by(|a, b| a.confidence.total_cmp(&b.confidence))
    .ok_or(PpdcError::new(400, ErrorType::ApiError, "No matching primary resource found".to_string()))?;

    Ok(matched_element)
}
