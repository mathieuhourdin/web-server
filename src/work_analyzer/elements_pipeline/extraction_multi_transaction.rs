use serde::{Deserialize, Serialize};

use crate::entities::error::PpdcError;
use crate::openai_handler::GptRequestConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct MultiTransactionExtraction {
    pub inputs: Vec<ExtractedInputEvent>,
    pub outputs: Vec<ExtractedOutputEvent>,
    pub internals: Vec<ExtractedInternalEvent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedInputEvent {
    pub target: InputTarget,
    pub verb: InputVerb,
    pub status: ExtractionStatus,
    pub date_offset: i32,
    pub evidences: Vec<String>,
    pub extractions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InputTarget {
    pub resource_identifier: String,
    pub author: Option<String>,
    pub theme: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedOutputEvent {
    pub target: OutputTarget,
    pub verb: OutputVerb,
    pub status: ExtractionStatus,
    pub date_offset: i32,
    pub evidences: Vec<String>,
    pub extractions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutputTarget {
    pub output_identifier: String,
    pub output_type: OutputType,
    pub theme: Option<String>,
    pub about_resource_identifier: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedInternalEvent {
    pub title: String,
    pub kind: InternalKind,
    pub polarity: InternalPolarity,
    pub intensity: i32,
    pub date_offset: i32,
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

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InputVerb {
    Read,
    Watch,
    Listen,
    Consult,
    Study,
    Skim,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutputVerb {
    Write,
    Draft,
    Revise,
    Code,
    Build,
    Send,
    Publish,
    Fix,
    Plan,
    Think,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutputType {
    Doc,
    Note,
    Code,
    Email,
    Message,
    Design,
    File,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InternalKind {
    Mood,
    Energy,
    Decision,
    Realization,
    Difficulty,
    Question,
    Intention,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InternalPolarity {
    Positive,
    Negative,
    Mixed,
    Neutral,
    Unknown,
}

pub async fn extract_multi_transaction_from_test_trace(
) -> Result<MultiTransactionExtraction, PpdcError> {
    let trace_text =
        include_str!("prompts/landmark_resource/extraction/multi_transaction_test_trace.md");
    let user_prompt = format!(
        "
        trace_text : \n {}
        ",
        trace_text
    );

    let schema = serde_json::from_str::<serde_json::Value>(include_str!(
        "prompts/landmark_resource/extraction/multi_transaction_schema.json"
    ))?;

    let request = GptRequestConfig::new(
        "gpt-4.1-nano".to_string(),
        include_str!("prompts/landmark_resource/extraction/multi_transaction_system.md")
            .to_string(),
        user_prompt,
        Some(schema),
        None,
    )
    .with_display_name("Elements / Extraction / Multi transaction / Playground");

    request.execute().await
}
