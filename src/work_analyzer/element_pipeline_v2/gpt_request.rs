use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

use crate::entities::error::PpdcError;
use crate::entities_v2::{landmark::Landmark, reference::Reference, trace_mirror::TraceMirror};
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::analysis_processor::AnalysisContext;

#[derive(Debug, Serialize)]
struct LandmarkSummary {
    title: String,
    content: String,
    landmark_type: String,
}

#[derive(Debug, Serialize)]
struct ReferencePromptItem {
    tag_id: i32,
    mention: String,
    landmark: LandmarkSummary,
}

#[derive(Debug, Serialize)]
struct GrammaticalPromptInput {
    trace_text: String,
    references: Vec<ReferencePromptItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrammaticalExtractionOutput {
    pub transactions: Vec<TransactionClaim>,
    pub descriptives: Vec<DescriptiveClaim>,
    pub normatives: Vec<NormativeClaim>,
    pub evaluatives: Vec<EvaluativeClaim>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionKind {
    Input,
    Output,
    Transformation,
}

impl TransactionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TransactionKind::Input => "INPUT",
            TransactionKind::Output => "OUTPUT",
            TransactionKind::Transformation => "TRANSFORMATION",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionClaim {
    pub id: String,
    pub verb: String,
    pub kind: TransactionKind,
    pub target: String,
    pub theme: String,
    pub status: String,
    pub scope: String,
    pub life_domain: String,
    pub date_offset: String,
    pub subtask_of: Option<String>,
    pub related_transactions: Vec<String>,
    #[serde(alias = "span")]
    pub spans: Vec<String>,
    pub references_tags_id: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DescriptiveKind {
    Unit,
    Question,
    Theme,
}

impl DescriptiveKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DescriptiveKind::Unit => "UNIT",
            DescriptiveKind::Question => "QUESTION",
            DescriptiveKind::Theme => "THEME",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DescriptiveClaim {
    pub id: String,
    pub object: String,
    pub kind: DescriptiveKind,
    pub unit_ids: Vec<String>,
    #[serde(alias = "span")]
    pub spans: Vec<String>,
    pub references_tags_id: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NormativeForce {
    Plan,
    Obligation,
    Recommendation,
    Principle,
}

impl NormativeForce {
    pub fn as_str(self) -> &'static str {
        match self {
            NormativeForce::Plan => "PLAN",
            NormativeForce::Obligation => "OBLIGATION",
            NormativeForce::Recommendation => "RECOMMENDATION",
            NormativeForce::Principle => "PRINCIPLE",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NormativeClaim {
    pub id: String,
    pub force: NormativeForce,
    pub polarity: String,
    pub applies_to: Vec<String>,
    #[serde(alias = "span")]
    pub spans: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvaluativeKind {
    Emotion,
    Energy,
    Quality,
    Interest,
}

impl EvaluativeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EvaluativeKind::Emotion => "EMOTION",
            EvaluativeKind::Energy => "ENERGY",
            EvaluativeKind::Quality => "QUALITY",
            EvaluativeKind::Interest => "INTEREST",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EvaluativeClaim {
    pub id: String,
    pub kind: EvaluativeKind,
    pub polarity: String,
    pub level: i32,
    pub descriptive_ids: Vec<String>,
    pub transaction_ids: Vec<String>,
    #[serde(alias = "span")]
    pub spans: Vec<String>,
}

pub async fn request_extraction(
    context: &AnalysisContext,
    trace_mirror: &TraceMirror,
) -> Result<(GrammaticalExtractionOutput, HashMap<i32, Uuid>), PpdcError> {
    let (prompt_input, tag_to_landmark_id) = build_prompt_input(context, trace_mirror)?;
    let system_prompt =
        include_str!("../../bin/prompts/gramatical_extraction/v2/system.md").to_string();
    let user_prompt = serde_json::to_string_pretty(&prompt_input)?;
    let schema = serde_json::from_str(include_str!(
        "../../bin/prompts/gramatical_extraction/v2/schema.json"
    ))?;

    let request = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        Some(context.analysis_id),
    )
    .with_display_name("Element Pipeline V2 / Grammatical Extraction");

    let raw_extraction: GrammaticalExtractionOutput = request.execute().await?;
    Ok((raw_extraction, tag_to_landmark_id))
}

fn build_prompt_input(
    context: &AnalysisContext,
    trace_mirror: &TraceMirror,
) -> Result<(GrammaticalPromptInput, HashMap<i32, Uuid>), PpdcError> {
    let references = Reference::find_for_trace_mirror(trace_mirror.id, &context.pool)?;
    let mut prompt_references = Vec::new();
    let mut tag_to_landmark_id = HashMap::new();

    for reference in references {
        let Some(landmark_id) = reference.landmark_id else {
            warn!(
                trace_mirror_id = %trace_mirror.id,
                tag_id = reference.tag_id,
                "Skipping reference without landmark_id"
            );
            continue;
        };

        let landmark = match Landmark::find(landmark_id, &context.pool) {
            Ok(landmark) => landmark,
            Err(err) => {
                warn!(
                    trace_mirror_id = %trace_mirror.id,
                    tag_id = reference.tag_id,
                    landmark_id = %landmark_id,
                    error = %err,
                    "Skipping reference because landmark hydration failed"
                );
                continue;
            }
        };

        if let Some(existing_landmark_id) = tag_to_landmark_id.get(&reference.tag_id) {
            if *existing_landmark_id != landmark_id {
                warn!(
                    trace_mirror_id = %trace_mirror.id,
                    tag_id = reference.tag_id,
                    existing_landmark_id = %existing_landmark_id,
                    new_landmark_id = %landmark_id,
                    "Duplicate tag_id mapped to different landmarks, keeping first mapping"
                );
                continue;
            }
        }

        tag_to_landmark_id.insert(reference.tag_id, landmark_id);
        prompt_references.push(ReferencePromptItem {
            tag_id: reference.tag_id,
            mention: reference.mention,
            landmark: LandmarkSummary {
                title: landmark.title,
                content: landmark.content,
                landmark_type: landmark.landmark_type.to_code().to_string(),
            },
        });
    }

    Ok((
        GrammaticalPromptInput {
            trace_text: trace_mirror.content.clone(),
            references: prompt_references,
        },
        tag_to_landmark_id,
    ))
}
