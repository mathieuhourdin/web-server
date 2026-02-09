use crate::entities_v2::landmark::LandmarkType;
use crate::work_analyzer::elements_pipeline::extraction::{
    ExtractedElements,
    ExtractionStatus,
    LandmarkSuggestion,
};
use serde::{Deserialize, Serialize};
use crate::entities::error::PpdcError;
use crate::entities_v2::landmark::Landmark;
use crate::work_analyzer::matching;
use crate::work_analyzer::matching::ElementWithIdentifier;
use crate::work_analyzer::analysis_processor::{AnalysisConfig, AnalysisContext, AnalysisInputs, AnalysisStateMirror};

/// Get the system prompt for matching based on landmark type
fn get_matching_system_prompt(landmark_type: LandmarkType) -> &'static str {
    match landmark_type {
        LandmarkType::Resource => include_str!("prompts/landmark_resource/matching/system.md"),
        LandmarkType::Theme => include_str!("prompts/landmark_theme/matching/system.md"),
        LandmarkType::Author => include_str!("prompts/landmark_author/matching/system.md"),
    }
}


pub async fn match_elements(
    config: &AnalysisConfig,
    context: &AnalysisContext,
    inputs: &AnalysisInputs,
    state: &AnalysisStateMirror,
    extracted: ExtractedElements,
) -> Result<MatchedElements, PpdcError> {
    run_matching_impl(config, context, inputs, state, extracted).await
}

async fn run_matching_impl(_config: &AnalysisConfig, context: &AnalysisContext, inputs: &AnalysisInputs, _state: &AnalysisStateMirror, extracted: ExtractedElements) -> Result<MatchedElements, PpdcError> {
    let log_header = format!("analysis_id: {}", context.analysis_id);
    let landmark_types = [LandmarkType::Resource, LandmarkType::Author, LandmarkType::Theme];

    let mut matching_placeholders = to_matched_placeholders(&extracted);

    for landmark_type in landmark_types {
        let display_name = match landmark_type {
            LandmarkType::Resource => "Elements / Matching / Resource",
            LandmarkType::Theme => "Elements / Matching / Theme",
            LandmarkType::Author => "Elements / Matching / Author",
        };
        let landmarks = inputs.previous_landscape_landmarks
            .iter()
            .filter(|landmark| landmark.landmark_type == landmark_type)
            .cloned()
            .collect::<Vec<Landmark>>();
        let landmark_match_placeholders = matching_placeholders
            .iter()
            .cloned()
            .filter_map(|element| {
                element
                    .landmark_suggestions
                    .iter()
                    .find(|suggestion| suggestion.landmark_type == landmark_type)
                    .map(|suggestion| LandmarkGptPayload::from_matched_element_and_suggestion(&element, suggestion))
            })
            .collect::<Vec<LandmarkGptPayload>>();
        let system_prompt = get_matching_system_prompt(landmark_type);
        let matching_results = matching::match_elements(
            landmark_match_placeholders.clone(),
            &landmarks,
            Some(system_prompt),
            context.analysis_id,
            display_name,
        ).await?;
        matching_placeholders = matching_placeholders
            .iter()
            .map(|element| {
                MatchedElement {
                    temporary_id: element.temporary_id.clone(),
                    title: element.title.clone(),
                    verb: element.verb.clone(),
                    status: element.status,
                    evidences: element.evidences.clone(),
                    extractions: element.extractions.clone(),
                    landmark_suggestions: element
                        .landmark_suggestions
                        .iter()
                        .map(|suggestion| {
                            if let Some(result) = matching_results
                                .iter()
                                .find(|result| result.element.temporary_id == suggestion.temporary_id)
                            {
                                LandmarkMatching {
                                    temporary_id: suggestion.temporary_id.clone(),
                                    matching_key: suggestion.matching_key.clone(),
                                    landmark_type: suggestion.landmark_type,
                                    candidate_id: result.candidate_id.clone(),
                                    confidence: result.confidence,
                                }
                            } else {
                                suggestion.clone()
                            }
                        })
                        .collect::<Vec<LandmarkMatching>>(),
                }
            })
            .collect::<Vec<MatchedElement>>();
    }
    tracing::info!(
        target: "work_analyzer",
        "{} trace_broker_matching_complete elements={}",
        log_header,
        matching_placeholders.len()
    );
    Ok(MatchedElements {
        elements: matching_placeholders,
    })
}

pub struct MatchedElements {
    pub elements: Vec<MatchedElement>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MatchedElement {
    pub temporary_id: String,
    pub title: String,
    pub verb: String,
    pub status: ExtractionStatus,
    pub evidences: Vec<String>,
    pub extractions: Vec<String>,
    pub landmark_suggestions: Vec<LandmarkMatching>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LandmarkMatching {
    pub temporary_id: String,
    pub matching_key: String,
    pub landmark_type: LandmarkType,
    pub candidate_id: Option<String>,
    pub confidence: f32,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LandmarkGptPayload {
    #[serde(skip_serializing)]
    pub temporary_id: String,
    pub matching_key: String,
    pub landmark_type: LandmarkType,
    pub title: String,
    pub evidences: Vec<String>,
    pub extractions: Vec<String>,
}

impl LandmarkGptPayload {
    pub fn from_matched_element_and_suggestion(element: &MatchedElement, suggestion: &LandmarkMatching) -> Self {
        Self {
            temporary_id: suggestion.temporary_id.clone(),
            matching_key: suggestion.matching_key.clone(),
            landmark_type: suggestion.landmark_type,
            title: element.title.clone(),
            evidences: element.evidences.clone(),
            extractions: element.extractions.clone(),
        }
    }
}
impl ElementWithIdentifier for LandmarkGptPayload {
    fn identifier(&self) -> String {
        self.matching_key.clone()
    }
}

impl ElementWithIdentifier for LandmarkMatching {
    fn identifier(&self) -> String {
        self.matching_key.clone()
    }
}

impl From<&LandmarkSuggestion> for LandmarkMatching {
    fn from(suggestion: &LandmarkSuggestion) -> Self {
        Self {
            temporary_id: suggestion.temporary_id.clone(),
            matching_key: suggestion.matching_key.clone(),
            landmark_type: suggestion.landmark_type,
            candidate_id: None,
            confidence: 0.0,
        }
    }
}

fn to_matched_placeholders(extracted: &ExtractedElements) -> Vec<MatchedElement> {
    extracted
        .elements
        .iter()
        .map(|element| MatchedElement {
            temporary_id: element.temporary_id.clone(),
            title: element.title.clone(),
            verb: element.verb.clone(),
            status: element.status,
            evidences: element.evidences.clone(),
            extractions: element.extractions.clone(),
            landmark_suggestions: element
                .landmark_suggestions
                .iter()
                .map(|suggestion| suggestion.into())
                .collect::<Vec<LandmarkMatching>>(),
        })
        .collect::<Vec<MatchedElement>>()
}

#[cfg(test)]
mod tests {
    use super::to_matched_placeholders;
    use crate::entities_v2::landmark::LandmarkType;
    use crate::work_analyzer::elements_pipeline::extraction::{
        ExtractedElement,
        ExtractedElements,
        ExtractionStatus,
        LandmarkSuggestion,
    };

    #[test]
    fn to_matched_placeholders_keeps_verb() {
        let extracted = ExtractedElements {
            elements: vec![ExtractedElement {
                temporary_id: "el-0".to_string(),
                title: "DONE - lire: Bullshit Jobs - Par David Graeber Sur travail".to_string(),
                verb: "DONE - lire".to_string(),
                status: ExtractionStatus::Done,
                evidences: vec!["Bullshit Jobs".to_string()],
                extractions: vec!["J'ai lu Bullshit Jobs".to_string()],
                landmark_suggestions: vec![LandmarkSuggestion {
                    temporary_id: "el-0-lm-resource-0".to_string(),
                    matching_key: "Bullshit Jobs".to_string(),
                    landmark_type: LandmarkType::Resource,
                }],
            }],
        };

        let placeholders = to_matched_placeholders(&extracted);

        assert_eq!(placeholders[0].verb, "DONE - lire");
        assert_eq!(placeholders[0].status, ExtractionStatus::Done);
    }
}
