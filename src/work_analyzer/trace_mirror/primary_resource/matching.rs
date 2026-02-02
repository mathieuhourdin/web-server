use crate::work_analyzer::matching;
use crate::entities_v2::landmark::Landmark;
use crate::entities::error::PpdcError;
use crate::work_analyzer::trace_mirror::primary_resource::suggestion::PrimaryResourceSuggestion;

pub struct PrimaryResourceMatched {
    pub resource_identifier: String,
    pub theme: Option<String>,
    pub author: Option<String>,
    pub evidence: Vec<String>,
    pub candidate_id: Option<String>,
    pub confidence: f32,
}


pub async fn run(element: PrimaryResourceSuggestion, landmarks: &Vec<Landmark>) -> Result<Vec<PrimaryResourceMatched>, PpdcError> {
    let matched_elements = matching::match_elements::<PrimaryResourceSuggestion>(vec![element], landmarks, None).await?;

    let matched_elements = matched_elements.iter().map(|element| PrimaryResourceMatched {
        resource_identifier: element.element.resource_identifier.clone(),
        theme: element.element.theme.clone(),
        author: element.element.author.clone(),
        evidence: element.element.evidence.clone(),
        candidate_id: element.candidate_id.clone(),
        confidence: element.confidence,
    }).collect();
    Ok(matched_elements)
}