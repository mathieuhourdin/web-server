use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

use crate::entities::error::PpdcError;
use crate::entities_v2::{
    landmark::{Landmark, LandmarkType},
    reference::Reference,
    trace_mirror::TraceMirror,
};
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
struct HighLevelProjectPromptItem {
    id: i32,
    title: String,
    subtitle: String,
    content: String,
    spans: Vec<String>,
}

#[derive(Debug, Serialize)]
struct GrammaticalPromptInput {
    trace_text: String,
    references: Vec<ReferencePromptItem>,
    high_level_projects: Vec<HighLevelProjectPromptItem>,
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
    #[serde(default)]
    pub high_level_project_ids: Vec<i32>,
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
    #[serde(default)]
    pub high_level_project_ids: Vec<i32>,
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

pub fn grammatical_extraction_system_prompt() -> String {
    include_str!("system.md").to_string()
}

pub fn grammatical_extraction_schema() -> Result<serde_json::Value, PpdcError> {
    Ok(serde_json::from_str(include_str!("schema.json"))?)
}

pub async fn request_extraction_from_prompts(
    analysis_id: Uuid,
    system_prompt: String,
    user_prompt: String,
) -> Result<GrammaticalExtractionOutput, PpdcError> {
    let schema = grammatical_extraction_schema()?;
    let request = GptRequestConfig::new(
        "gpt-4.1-mini".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        Some(analysis_id),
    )
    .with_display_name("Element Pipeline V2 / Grammatical Extraction");
    request.execute().await
}

pub async fn request_extraction_with_prompts(
    context: &AnalysisContext,
    trace_mirror: &TraceMirror,
) -> Result<
    (
        GrammaticalExtractionOutput,
        HashMap<i32, Uuid>,
        HashMap<i32, Uuid>,
        String,
        String,
    ),
    PpdcError,
> {
    let (prompt_input, tag_to_landmark_id, hlp_id_to_uuid) =
        build_prompt_input(context, trace_mirror)?;
    let system_prompt = grammatical_extraction_system_prompt();
    let user_prompt = serde_json::to_string_pretty(&prompt_input)?;
    let raw_extraction = request_extraction_from_prompts(
        context.analysis_id,
        system_prompt.clone(),
        user_prompt.clone(),
    )
    .await?;
    Ok((
        raw_extraction,
        tag_to_landmark_id,
        hlp_id_to_uuid,
        system_prompt,
        user_prompt,
    ))
}

fn build_prompt_input(
    context: &AnalysisContext,
    trace_mirror: &TraceMirror,
) -> Result<
    (
        GrammaticalPromptInput,
        HashMap<i32, Uuid>,
        HashMap<i32, Uuid>,
    ),
    PpdcError,
> {
    let references = Reference::find_for_trace_mirror(trace_mirror.id, &context.pool)?;
    let mut prompt_references = Vec::new();
    let mut tag_to_landmark_id = HashMap::new();
    let mut hlp_id_to_uuid = HashMap::new();
    let mut high_level_projects: Vec<HighLevelProjectPromptItem> = Vec::new();
    let mut high_level_project_index_by_landmark_id: HashMap<Uuid, usize> = HashMap::new();

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
        if landmark.landmark_type == LandmarkType::HighLevelProject {
            if let Some(existing_index) =
                high_level_project_index_by_landmark_id.get(&landmark_id).copied()
            {
                let spans = &mut high_level_projects[existing_index].spans;
                if !spans.iter().any(|span| span == &reference.mention) {
                    spans.push(reference.mention.clone());
                }
            } else {
                let hlp_prompt_id = high_level_projects.len() as i32;
                high_level_project_index_by_landmark_id
                    .insert(landmark_id, high_level_projects.len());
                hlp_id_to_uuid.insert(hlp_prompt_id, landmark_id);
                high_level_projects.push(HighLevelProjectPromptItem {
                    id: hlp_prompt_id,
                    title: landmark.title.clone(),
                    subtitle: landmark.subtitle.clone(),
                    content: landmark.content.clone(),
                    spans: vec![reference.mention.clone()],
                });
            }
        }

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
            high_level_projects,
        },
        tag_to_landmark_id,
        hlp_id_to_uuid,
    ))
}
