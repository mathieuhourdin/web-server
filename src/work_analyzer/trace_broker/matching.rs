use crate::entities_v2::landmark::LandmarkType;
use crate::work_analyzer::trace_broker::traits::ProcessorContext;
use crate::work_analyzer::trace_broker::extraction::{ExtractedElements, LandmarkSuggestion};
use serde::{Deserialize, Serialize};
use crate::entities::error::PpdcError;
use crate::entities_v2::landmark::Landmark;
use crate::work_analyzer::matching;
use crate::work_analyzer::matching::ElementWithIdentifier;
use crate::work_analyzer::analysis_processor::{AnalysisConfig, AnalysisContext, AnalysisInputs};

/// Get the system prompt for matching based on landmark type
fn get_matching_system_prompt(landmark_type: LandmarkType) -> &'static str {
    match landmark_type {
        LandmarkType::Resource => include_str!("prompts/landmark_resource/matching/system.md"),
        LandmarkType::Theme => include_str!("prompts/landmark_theme/matching/system.md"),
        LandmarkType::Author => include_str!("prompts/landmark_author/matching/system.md"),
    }
}


pub async fn match_elements(
    _config: &AnalysisConfig,
    _context: &AnalysisContext,
    _inputs: &AnalysisInputs,
    extracted: ExtractedElements,
) -> Result<MatchedElements, PpdcError> {
    run_matching_impl(extracted).await
}

async fn run_matching_impl(extracted: ExtractedElements) -> Result<MatchedElements, PpdcError> {
    let log_header = format!("analysis_id: {}", extracted.context.landscape_analysis_id);
    let landmark_types = [LandmarkType::Resource, LandmarkType::Author, LandmarkType::Theme];

    tracing::info!(
        target: "work_analyzer",
        "{} trace_broker_matching_start elements={}",
        log_header,
        extracted.elements.len()
    );

    let mut matching_placeholders = extracted
        .elements
        .iter()
        .map(|element| {
            MatchedElement {
                temporary_id: element.temporary_id.clone(),
                title: element.title.clone(),
                evidence: element.evidence.clone(),
                extractions: element.extractions.clone(),
                landmark_suggestions: element
                    .landmark_suggestions
                    .iter()
                    .map(|suggestion| suggestion.into())
                    .collect::<Vec<LandmarkMatching>>(),
            }
        })
        .collect::<Vec<MatchedElement>>();

    for landmark_type in landmark_types {
        let landmarks = extracted
            .context
            .landmarks
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
        tracing::info!(
            target: "work_analyzer",
            "{} trace_broker_matching_request landmark_type={:?} candidates={} elements={}",
            log_header,
            landmark_type,
            landmarks.len(),
            landmark_match_placeholders.len()
        );
        let matching_results = matching::match_elements(
            landmark_match_placeholders.clone(),
            &landmarks,
            Some(system_prompt),
            Some(&log_header),
        ).await?;
        matching_placeholders = matching_placeholders
            .iter()
            .map(|element| {
                MatchedElement {
                    temporary_id: element.temporary_id.clone(),
                    title: element.title.clone(),
                    evidence: element.evidence.clone(),
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
        context: extracted.context,
        elements: matching_placeholders,
    })
}

impl ExtractedElements {
    /**
     * This function is really dirty right now, a lot of data clone.
     * Should not be really a problem since the amount of elements is not that big.
     * I may come back later to improve it.
     */
    pub async fn run_matching(self) -> Result<MatchedElements, PpdcError> {
        run_matching_impl(self).await
    }
}


pub struct MatchedElements {
    pub context: ProcessorContext,
    pub elements: Vec<MatchedElement>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MatchedElement {
    pub temporary_id: String,
    pub title: String,
    pub evidence: String,
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
    pub evidence: String,
    pub extractions: Vec<String>,
}

impl LandmarkGptPayload {
    pub fn from_matched_element_and_suggestion(element: &MatchedElement, suggestion: &LandmarkMatching) -> Self {
        Self {
            temporary_id: suggestion.temporary_id.clone(),
            matching_key: suggestion.matching_key.clone(),
            landmark_type: suggestion.landmark_type,
            title: element.title.clone(),
            evidence: element.evidence.clone(),
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
