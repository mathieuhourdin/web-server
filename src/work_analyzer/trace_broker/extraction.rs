use crate::entities::error::PpdcError;
use crate::work_analyzer::trace_broker::traits::ProcessorContext;
use crate::entities_v2::landmark::LandmarkType;
use crate::openai_handler::GptRequestConfig;
use serde::{Deserialize, Serialize};

pub struct Extractable {
    pub context: ProcessorContext,
}

impl Extractable {
    pub async fn extract(self) -> Result<ExtractedElements, PpdcError> {
        let elements = extract_input_elements(&self.context).await?;
        Ok(ExtractedElements {
            context: self.context,
            elements,
        })
    }
}

impl From<ProcessorContext> for Extractable {
    fn from(context: ProcessorContext) -> Self {
        Self {
            context: context,
        }
    }
}
pub struct ExtractedElements {
    pub context: ProcessorContext,
    pub elements: Vec<ExtractedElement>,
}
pub struct ExtractedElement {
    pub temporary_id: String,
    pub title: String,
    pub evidence: String,
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
    pub evidence: String,
    pub extractions: Vec<String>,
}

impl ExtractedInputElement {
    fn to_extracted_element(&self, temporary_id: String) -> ExtractedElement {
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
            title: format!("{:?} - Par {:?} Sur {:?}", self.resource_identifier.clone().unwrap_or_default(), self.author.clone().unwrap_or_default(), self.theme.clone().unwrap_or_default()),
            evidence: self.evidence.clone(),    
            extractions: self.extractions.clone(),
            landmark_suggestions,
        }
    }
}

pub async fn extract_input_elements(context: &ProcessorContext) -> Result<Vec<ExtractedElement>, PpdcError> {
    let log_header = format!("analysis_id: {}", context.landscape_analysis_id);
    tracing::info!(
        target: "work_analyzer",
        "{} trace_broker_extraction_start trace_id={} trace_mirror_id={}",
        log_header,
        context.trace.id,
        context.trace_mirror.id
    );
    let trace_string = format!("{}\n{}\n{}", context.trace_mirror.title, context.trace_mirror.subtitle, context.trace_mirror.content);
    let user_prompt = format!("
    trace_text : \n {}
    ",
    trace_string);
    let gpt_request_config = GptRequestConfig::new(
        "gpt-4.1-nano".to_string(),
        include_str!("prompts/landmark_resource/extraction/system.md").to_string(),
        &user_prompt,
        Some(serde_json::from_str::<serde_json::Value>(include_str!("prompts/landmark_resource/extraction/schema.json"))?),
        Some(context.landscape_analysis_id),
    ).with_log_header(log_header.as_str());
    let elements: ExtractedInputElements = gpt_request_config.execute().await?;
    tracing::info!(
        target: "work_analyzer",
        "{} trace_broker_extraction_complete elements={}",
        log_header,
        elements.elements.len()
    );
    Ok(elements.elements.into_iter().enumerate().map(|(index, element)| element.to_extracted_element(format!("el-{}", index))).collect())
}
