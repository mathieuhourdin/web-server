use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::entities::error::PpdcError;
use crate::entities::resource::{MaturingState};
use crate::entities_v2::{
    landmark::{Landmark, LandmarkType, LandmarkWithParentsAndElements, NewLandmark},
    element::model::Element,
};
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::analysis_processor::AnalysisContext;
use crate::work_analyzer::elements_pipeline::creation::{
    CreatedElements,
    NewAuthorLandmark,
    NewResourceLandmark,
    NewThemeLandmark,
};
use crate::work_analyzer::elements_pipeline::traits::NewLandmarkForExtractedElement;
use crate::work_analyzer::elements_pipeline::types::IdentityState;

#[derive(Debug, Serialize)]
struct LandmarkContext {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub maturing_state: MaturingState,
}

impl From<&Landmark> for LandmarkContext {
    fn from(landmark: &Landmark) -> Self {
        Self {
            title: landmark.title.clone(),
            subtitle: landmark.subtitle.clone(),
            content: landmark.content.clone(),
            maturing_state: landmark.maturing_state,
        }
    }
}

#[derive(Debug, Serialize)]
struct ElementContext {
    pub title: String,
    pub subtitle: String,
    pub content: String,
}

impl From<&Element> for ElementContext {
    fn from(element: &Element) -> Self {
        Self {
            title: element.title.clone(),
            subtitle: element.subtitle.clone(),
            content: element.content.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
struct LandmarkRefinementInput {
    pub matching_key: String,
    pub element_title: String,
    pub evidences: Vec<String>,
    pub extractions: Vec<String>,
    pub existing_landmark: LandmarkContext,
    pub parent_landmarks: Vec<LandmarkContext>,
    pub related_elements: Vec<ElementContext>,
}

fn dedup_keep_order(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn parse_element_payload(content: &str) -> (Vec<String>, Vec<String>) {
    let Some(after_evidences) = content.strip_prefix("Evidences: ") else {
        return (vec![], vec![]);
    };

    let mut split = after_evidences.splitn(2, "\n\nExtractions: ");
    let evidences_raw = split.next().unwrap_or("").trim();
    let extractions_raw = split.next().unwrap_or("").trim();

    let evidences = if evidences_raw.is_empty() || evidences_raw == "[]" {
        vec![]
    } else {
        evidences_raw
            .split(", ")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<String>>()
    };

    let extractions = if extractions_raw.is_empty() || extractions_raw == "[]" {
        vec![]
    } else {
        vec![extractions_raw.to_string()]
    };

    (evidences, extractions)
}

fn build_refinement_input(
    landmark: &Landmark,
    context: &LandmarkWithParentsAndElements,
    linked_elements: &[Element],
) -> LandmarkRefinementInput {
    let mut evidences = Vec::new();
    let mut extractions = Vec::new();

    for element in linked_elements {
        let (parsed_evidences, parsed_extractions) = parse_element_payload(&element.content);
        evidences.extend(parsed_evidences);
        extractions.extend(parsed_extractions);
    }

    if evidences.is_empty() {
        evidences = linked_elements
            .iter()
            .map(|element| element.title.clone())
            .collect::<Vec<String>>();
    }

    if extractions.is_empty() {
        extractions = linked_elements
            .iter()
            .map(|element| element.content.clone())
            .collect::<Vec<String>>();
    }

    LandmarkRefinementInput {
        matching_key: landmark.title.clone(),
        element_title: linked_elements
            .first()
            .map(|element| element.title.clone())
            .unwrap_or_else(|| landmark.title.clone()),
        evidences: dedup_keep_order(evidences),
        extractions: dedup_keep_order(extractions),
        existing_landmark: LandmarkContext::from(&context.landmark),
        parent_landmarks: context
            .parent_landmarks
            .iter()
            .map(LandmarkContext::from)
            .collect(),
        related_elements: context
            .related_elements
            .iter()
            .map(|element| ElementContext::from(element))
            .collect(),
    }
}

fn get_refinement_system_prompt_for_type(landmark_type: LandmarkType) -> String {
    match landmark_type {
        LandmarkType::Resource => include_str!("prompts/landmark_resource/refinement/system.md").to_string(),
        LandmarkType::Theme => include_str!("prompts/landmark_theme/refinement/system.md").to_string(),
        LandmarkType::Author => include_str!("prompts/landmark_author/refinement/system.md").to_string(),
    }
}

fn get_schema_for_type(landmark_type: LandmarkType) -> serde_json::Value {
    match landmark_type {
        LandmarkType::Resource => {
            serde_json::from_str(include_str!("prompts/landmark_resource/creation/schema.json")).unwrap()
        }
        LandmarkType::Theme => {
            serde_json::from_str(include_str!("prompts/landmark_theme/creation/schema.json")).unwrap()
        }
        LandmarkType::Author => {
            serde_json::from_str(include_str!("prompts/landmark_author/creation/schema.json")).unwrap()
        }
    }
}

async fn refine_landmark_via_gpt(
    context: &AnalysisContext,
    landmark_type: LandmarkType,
    parent_landmark_id: uuid::Uuid,
    input: &LandmarkRefinementInput,
) -> Result<Option<NewLandmark>, PpdcError> {
    let user_prompt = serde_json::to_string_pretty(input)?;
    let system_prompt = get_refinement_system_prompt_for_type(landmark_type);
    let schema = get_schema_for_type(landmark_type);
    let display_name = match landmark_type {
        LandmarkType::Resource => "Elements / Refinement / Resource",
        LandmarkType::Theme => "Elements / Refinement / Theme",
        LandmarkType::Author => "Elements / Refinement / Author",
    };

    let gpt_config = GptRequestConfig::new(
        "gpt-4.1-nano".to_string(),
        &system_prompt,
        &user_prompt,
        Some(schema),
        Some(context.analysis_id),
    )
    .with_display_name(display_name);

    let (new_landmark, identity_state) = match landmark_type {
        LandmarkType::Resource => {
            let result: NewResourceLandmark = gpt_config.execute().await?;
            (
                result.to_new_landmark(
                    context.analysis_id,
                    context.user_id,
                    Some(parent_landmark_id),
                ),
                result.identity_state(),
            )
        }
        LandmarkType::Theme => {
            let result: NewThemeLandmark = gpt_config.execute().await?;
            (
                result.to_new_landmark(
                    context.analysis_id,
                    context.user_id,
                    Some(parent_landmark_id),
                ),
                result.identity_state(),
            )
        }
        LandmarkType::Author => {
            let result: NewAuthorLandmark = gpt_config.execute().await?;
            (
                result.to_new_landmark(
                    context.analysis_id,
                    context.user_id,
                    Some(parent_landmark_id),
                ),
                result.identity_state(),
            )
        }
    };

    if matches!(identity_state, IdentityState::Identified) {
        Ok(Some(new_landmark))
    } else {
        Ok(None)
    }
}

fn apply_refinement(
    existing: Landmark,
    refined: NewLandmark,
    pool: &crate::db::DbPool,
) -> Result<Landmark, PpdcError> {
    let updated = Landmark {
        title: refined.title,
        subtitle: refined.subtitle,
        content: refined.content,
        landmark_type: refined.landmark_type,
        maturing_state: refined.maturing_state,
        ..existing
    };
    updated.update(pool)
}

pub async fn refine_landmarks(
    context: &AnalysisContext,
    created: CreatedElements,
) -> Result<CreatedElements, PpdcError> {
    if created.created_landmarks.is_empty() {
        return Ok(created);
    }

    let mut updated_by_id: HashMap<uuid::Uuid, Landmark> = created
        .created_landmarks
        .iter()
        .map(|landmark| (landmark.id, landmark.clone()))
        .collect();
    let mut processed_landmark_ids = HashSet::new();

    for landmark in &created.created_landmarks {
        if !processed_landmark_ids.insert(landmark.id) {
            continue;
        }

        if landmark.maturing_state != MaturingState::Draft {
            continue;
        }

        let analysis = match landmark.clone().get_analysis(&context.pool) {
            Ok(analysis) => analysis,
            Err(err) => {
                tracing::warn!(
                    target: "work_analyzer",
                    "analysis_id: {} refinement_skip_no_analysis landmark_id={} error={}",
                    context.analysis_id,
                    landmark.id,
                    err
                );
                continue;
            }
        };
        if analysis.id != context.analysis_id {
            continue;
        }

        let Some(parent_landmark) = landmark.find_parent(&context.pool)? else {
            continue;
        };

        let linked_elements = landmark.find_elements(&context.pool)?;
        let landmark_context = Landmark::find_with_parents(landmark.id, &context.pool)?;
        let refinement_input = build_refinement_input(landmark, &landmark_context, &linked_elements);

        match refine_landmark_via_gpt(
            context,
            landmark.landmark_type,
            parent_landmark.id,
            &refinement_input,
        )
        .await
        {
            Ok(Some(refined_new_landmark)) => {
                let existing = updated_by_id
                    .get(&landmark.id)
                    .cloned()
                    .unwrap_or_else(|| landmark.clone());
                let updated = apply_refinement(existing, refined_new_landmark, &context.pool)?;
                updated_by_id.insert(updated.id, updated);
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(
                    target: "work_analyzer",
                    "analysis_id: {} refinement_failed landmark_id={} error={}",
                    context.analysis_id,
                    landmark.id,
                    err
                );
            }
        }
    }

    let updated_landmarks = created
        .created_landmarks
        .iter()
        .map(|landmark| {
            updated_by_id
                .get(&landmark.id)
                .cloned()
                .unwrap_or_else(|| landmark.clone())
        })
        .collect::<Vec<Landmark>>();

    Ok(CreatedElements {
        created_landmarks: updated_landmarks,
        created_elements: created.created_elements,
    })
}
