use crate::entities::error::PpdcError;
use crate::entities_v2::landmark::LandmarkType;
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::analysis_processor::{AnalysisConfig, AnalysisContext, AnalysisInputs, AnalysisStateMirror};
use serde::{Deserialize, Serialize};

pub async fn extract_elements(
    _config: &AnalysisConfig,
    context: &AnalysisContext,
    inputs: &AnalysisInputs,
    state: &AnalysisStateMirror,
) -> Result<ExtractedElements, PpdcError> {
    let elements = extract_input_elements(_config, context, inputs, state).await?;
    Ok(ExtractedElements {
        elements,
    })
}


pub struct ExtractedElements {
    pub elements: Vec<ExtractedElement>,
}
pub struct ExtractedElement {
    pub temporary_id: String,
    pub title: String,
    pub verb: String,
    pub status: ExtractionStatus,
    pub evidences: Vec<String>,
    pub extractions: Vec<String>,
    pub landmark_suggestions: Vec<LandmarkSuggestion>,
}
pub struct LandmarkSuggestion {
    pub temporary_id: String,
    pub matching_key: String,
    pub landmark_type: LandmarkType,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtractedInputElements {
    pub elements: Vec<ExtractedInputElement>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtractedInputElement {
    pub resource_identifier: Option<String>,
    pub author: Option<String>,
    pub theme: Option<String>,
    pub verb: String,
    pub status: ExtractionStatus,
    pub evidences: Vec<String>,
    pub extractions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExtractionStatus {
    Done,
    Intended,
    InProgress,
    Unknown,
}

impl ExtractionStatus {
    fn to_label(self) -> &'static str {
        match self {
            ExtractionStatus::Done => "DONE",
            ExtractionStatus::Intended => "INTENDED",
            ExtractionStatus::InProgress => "IN_PROGRESS",
            ExtractionStatus::Unknown => "UNKNOWN",
        }
    }
}

impl ExtractedInputElement {
    fn normalize_verb(verb: &str) -> String {
        let trimmed_verb = verb.trim();
        if trimmed_verb.is_empty() {
            "unknown".to_string()
        } else {
            trimmed_verb.to_string()
        }
    }

    fn compose_status_verb(status: ExtractionStatus, verb: &str) -> String {
        format!("{} - {}", status.to_label(), verb)
    }

    fn to_extracted_element(&self, temporary_id: String) -> ExtractedElement {
        let normalized_verb = Self::normalize_verb(&self.verb);
        let status_verb = Self::compose_status_verb(self.status, &normalized_verb);
        let mut landmark_suggestions = vec![];
        let mut counter = 0;
        if let Some(theme) = self.theme.clone() {
            if !theme.is_empty() {
                landmark_suggestions.push(LandmarkSuggestion {
                    temporary_id: format!("{}-lm-theme-{}", temporary_id, counter),
                    matching_key: theme,
                    landmark_type: LandmarkType::Theme,
                });
                counter += 1;
            }
        }
        if let Some(resource_identifier) = self.resource_identifier.clone() {
            if !resource_identifier.is_empty() {
            landmark_suggestions.push(LandmarkSuggestion {
                    temporary_id: format!("{}-lm-resource-{}", temporary_id, counter),
                    matching_key: resource_identifier,
                    landmark_type: LandmarkType::Resource,
                });
            }
        }
        if let Some(author) = self.author.clone() {
            if !author.is_empty() {
                landmark_suggestions.push(LandmarkSuggestion {
                    temporary_id: format!("{}-lm-author-{}", temporary_id, counter),
                    matching_key: author,
                    landmark_type: LandmarkType::Author,
                });
            }
        }
        ExtractedElement {
            temporary_id: temporary_id,
            title: format!(
                "{}: {} - Par {} Sur {}",
                status_verb,
                self.resource_identifier.clone().unwrap_or_default(),
                self.author.clone().unwrap_or_default(),
                self.theme.clone().unwrap_or_default()
            ),
            verb: status_verb,
            status: self.status,
            evidences: self.evidences.clone(),
            extractions: self.extractions.clone(),
            landmark_suggestions,
        }
    }
}

pub async fn extract_input_elements(_config: &AnalysisConfig, context: &AnalysisContext, _inputs: &AnalysisInputs, state: &AnalysisStateMirror) -> Result<Vec<ExtractedElement>, PpdcError> {
    let trace_string = format!("{}\n{}\n{}", state.trace_mirror.title, state.trace_mirror.subtitle, state.trace_mirror.content);
    let user_prompt = format!("
    trace_text : \n {}
    ",
    trace_string);
    let gpt_request_config = GptRequestConfig::new(
        "gpt-4.1-nano".to_string(),
        include_str!("prompts/landmark_resource/extraction/system.md").to_string(),
        &user_prompt,
        Some(serde_json::from_str::<serde_json::Value>(include_str!("prompts/landmark_resource/extraction/schema.json"))?),
        Some(context.analysis_id),
    )
    .with_display_name("Elements / Extraction");
    let elements: ExtractedInputElements = gpt_request_config.execute().await?;
    Ok(elements.elements.into_iter().enumerate().map(|(index, element)| element.to_extracted_element(format!("el-{}", index))).collect())
}

#[cfg(test)]
mod tests {
    use super::{ExtractedInputElement, ExtractionStatus};

    #[test]
    fn to_extracted_element_includes_normalized_verb_in_field_and_title() {
        let extracted_input = ExtractedInputElement {
            resource_identifier: Some("Bullshit Jobs".to_string()),
            author: Some("David Graeber".to_string()),
            theme: Some("travail".to_string()),
            verb: " lire ".to_string(),
            status: ExtractionStatus::Done,
            evidences: vec!["Bullshit Jobs".to_string()],
            extractions: vec!["J'ai commencé à lire".to_string()],
        };

        let extracted = extracted_input.to_extracted_element("el-0".to_string());

        assert_eq!(extracted.verb, "DONE - lire");
        assert_eq!(extracted.status, ExtractionStatus::Done);
        assert!(extracted.title.starts_with("DONE - lire:"));
    }
}
