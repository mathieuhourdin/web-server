use crate::entities::error::PpdcError;
use crate::entities_v2::{
    element::{link_to_landmark, Element, ElementType, NewElement},
    landmark::{self, Landmark, LandmarkType, NewLandmark},
    landscape_analysis,
};
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::analysis_processor::{
    AnalysisConfig, AnalysisContext, AnalysisInputs, AnalysisStateMirror,
};
use crate::work_analyzer::elements_pipeline::matching::{
    LandmarkMatching, MatchedElement, MatchedElements,
};
use crate::work_analyzer::elements_pipeline::traits::NewLandmarkForExtractedElement;
use crate::work_analyzer::elements_pipeline::types::IdentityState;
use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// GPT Input Struct
// ============================================================================

/// Input struct for GPT landmark creation calls.
/// Combines landmark matching data with element context.
#[derive(Debug, Serialize)]
pub struct LandmarkCreationInput {
    /// From LandmarkMatching - the identifier for the landmark
    pub matching_key: String,
    /// From LandmarkMatching - the type of landmark to create
    pub landmark_type: LandmarkType,
    /// From parent MatchedElement - title of the element
    pub element_title: String,
    /// From parent MatchedElement - evidences/citations from the trace
    pub evidences: Vec<String>,
    /// From parent MatchedElement - extracted insights
    pub extractions: Vec<String>,
}

impl LandmarkCreationInput {
    pub fn from_matched(element: &MatchedElement, suggestion: &LandmarkMatching) -> Self {
        Self {
            matching_key: suggestion.matching_key.clone(),
            landmark_type: suggestion.landmark_type,
            element_title: element.title.clone(),
            evidences: element.evidences.clone(),
            extractions: element.extractions.clone(),
        }
    }
}

// ============================================================================
// GPT Output Structs (per landmark type)
// ============================================================================

/// GPT output for Resource landmark creation
#[derive(Debug, Deserialize)]
pub struct NewResourceLandmark {
    pub title: String,
    pub author: String,
    pub content: String,
    pub identity_state: IdentityState,
}

impl NewLandmarkForExtractedElement for NewResourceLandmark {
    fn title(&self) -> String {
        self.title.clone()
    }
    fn subtitle(&self) -> String {
        format!("Par {}", self.author)
    }
    fn content(&self) -> String {
        self.content.clone()
    }
    fn identity_state(&self) -> IdentityState {
        self.identity_state
    }
    fn landmark_type(&self) -> LandmarkType {
        LandmarkType::Resource
    }
}

/// GPT output for Theme landmark creation
#[derive(Debug, Deserialize)]
pub struct NewThemeLandmark {
    pub title: String,
    pub content: String,
    pub identity_state: IdentityState,
}

impl NewLandmarkForExtractedElement for NewThemeLandmark {
    fn title(&self) -> String {
        self.title.clone()
    }
    fn subtitle(&self) -> String {
        String::new()
    }
    fn content(&self) -> String {
        self.content.clone()
    }
    fn identity_state(&self) -> IdentityState {
        self.identity_state
    }
    fn landmark_type(&self) -> LandmarkType {
        LandmarkType::Topic
    }
}

/// GPT output for Author landmark creation
#[derive(Debug, Deserialize)]
pub struct NewAuthorLandmark {
    pub title: String,
    pub main_subjects: String,
    pub content: String,
    pub identity_state: IdentityState,
}

impl NewLandmarkForExtractedElement for NewAuthorLandmark {
    fn title(&self) -> String {
        self.title.clone()
    }
    fn subtitle(&self) -> String {
        self.main_subjects.clone()
    }
    fn content(&self) -> String {
        self.content.clone()
    }
    fn identity_state(&self) -> IdentityState {
        self.identity_state
    }
    fn landmark_type(&self) -> LandmarkType {
        LandmarkType::Person
    }
}

// ============================================================================
// Result Struct
// ============================================================================

/// Result of the creation process
pub struct CreatedElements {
    pub created_landmarks: Vec<Landmark>,
    pub created_elements: Vec<Element>,
}

// ============================================================================
// Prompt and Schema Helpers
// ============================================================================

fn get_system_prompt_for_type(landmark_type: LandmarkType) -> String {
    match landmark_type {
        LandmarkType::Resource => {
            include_str!("prompts/landmark_resource/creation/system.md").to_string()
        }
        LandmarkType::Topic => {
            include_str!("prompts/landmark_theme/creation/system.md").to_string()
        }
        LandmarkType::Person => {
            include_str!("prompts/landmark_author/creation/system.md").to_string()
        }
        _ => include_str!("prompts/landmark_resource/creation/system.md").to_string(),
    }
}

fn get_schema_for_type(landmark_type: LandmarkType) -> serde_json::Value {
    match landmark_type {
        LandmarkType::Resource => serde_json::from_str(include_str!(
            "prompts/landmark_resource/creation/schema.json"
        ))
        .unwrap(),
        LandmarkType::Topic => {
            serde_json::from_str(include_str!("prompts/landmark_theme/creation/schema.json"))
                .unwrap()
        }
        LandmarkType::Person => {
            serde_json::from_str(include_str!("prompts/landmark_author/creation/schema.json"))
                .unwrap()
        }
        _ => serde_json::from_str(include_str!(
            "prompts/landmark_resource/creation/schema.json"
        ))
        .unwrap(),
    }
}

// ============================================================================
// GPT Landmark Creation
// ============================================================================

/// Create a landmark via GPT based on the input and landmark type
async fn create_landmark_via_gpt(
    context: &AnalysisContext,
    input: &LandmarkCreationInput,
) -> Result<Landmark, PpdcError> {
    let user_prompt = serde_json::to_string_pretty(input)?;
    let system_prompt = get_system_prompt_for_type(input.landmark_type);
    let schema = get_schema_for_type(input.landmark_type);
    let display_name = match input.landmark_type {
        LandmarkType::Resource => "Elements / Creation / Resource",
        LandmarkType::Topic => "Elements / Creation / Topic",
        LandmarkType::Person => "Elements / Creation / Person",
        _ => "Elements / Creation / Resource",
    };

    let gpt_config = GptRequestConfig::new(
        "gpt-4.1-nano".to_string(),
        &system_prompt,
        &user_prompt,
        Some(schema),
        Some(context.analysis_id),
    )
    .with_display_name(display_name);

    // Call GPT and deserialize based on type
    let new_landmark: NewLandmark = match input.landmark_type {
        LandmarkType::Resource => {
            let result: NewResourceLandmark = gpt_config.execute().await?;
            result.to_new_landmark(context.analysis_id, context.user_id, None)
        }
        LandmarkType::Topic => {
            let result: NewThemeLandmark = gpt_config.execute().await?;
            result.to_new_landmark(context.analysis_id, context.user_id, None)
        }
        LandmarkType::Person => {
            let result: NewAuthorLandmark = gpt_config.execute().await?;
            result.to_new_landmark(context.analysis_id, context.user_id, None)
        }
        _ => {
            let result: NewResourceLandmark = gpt_config.execute().await?;
            result.to_new_landmark(context.analysis_id, context.user_id, None)
        }
    };

    let created_landmark = new_landmark.create(&context.pool)?;
    Ok(created_landmark)
}

// ============================================================================
// Main Creation Logic
// ============================================================================

pub async fn create_elements(
    config: &AnalysisConfig,
    context: &AnalysisContext,
    inputs: &AnalysisInputs,
    state: &AnalysisStateMirror,
    matched: MatchedElements,
) -> Result<CreatedElements, PpdcError> {
    run_creation_impl(config, context, inputs, state, matched).await
}

async fn run_creation_impl(
    config: &AnalysisConfig,
    context: &AnalysisContext,
    inputs: &AnalysisInputs,
    state: &AnalysisStateMirror,
    matched: MatchedElements,
) -> Result<CreatedElements, PpdcError> {
    let log_header = format!("analysis_id: {}", context.analysis_id);

    let mut created_landmarks: Vec<Landmark> = vec![];
    let mut created_elements: Vec<Element> = vec![];

    // Track which parent landmarks were updated (to avoid duplicate refs)
    let mut updated_landmark_ids: Vec<Uuid> = vec![];
    // Map from parent landmark ID to created child landmark ID
    let mut landmarks_updates: HashMap<Uuid, Uuid> = HashMap::new();

    for element in &matched.elements {
        let new_element = build_new_element_from_matched(
            element,
            inputs.trace.interaction_date,
            inputs.trace.id,
            state.trace_mirror.id,
            context.analysis_id,
            context.user_id,
        );
        let created_element = new_element.create(&context.pool)?;
        created_elements.push(created_element.clone());

        // Process each landmark suggestion for this element
        for suggestion in &element.landmark_suggestions {
            let created_landmark = process_suggestion(
                &config,
                &context,
                inputs,
                state,
                element,
                suggestion,
                &mut landmarks_updates,
                &mut updated_landmark_ids,
            )
            .await?;

            link_to_landmark(
                created_element.id,
                created_landmark.id,
                context.user_id,
                &context.pool,
            )?;
            created_landmarks.push(created_landmark);
        }
    }

    // Add landmark refs for landmarks that were not updated
    for landmark in &inputs.previous_landscape_landmarks {
        if !updated_landmark_ids.contains(&landmark.id) {
            landscape_analysis::add_landmark_ref(
                context.analysis_id,
                landmark.id,
                context.user_id,
                &context.pool,
            )?;
        }
    }

    tracing::info!(
        target: "work_analyzer",
        "{} trace_broker_creation_complete created_landmarks={} created_elements={}",
        log_header,
        created_landmarks.len(),
        created_elements.len()
    );
    Ok(CreatedElements {
        created_landmarks,
        created_elements,
    })
}

/// Process a single landmark suggestion
async fn process_suggestion(
    config: &AnalysisConfig,
    context: &AnalysisContext,
    _inputs: &AnalysisInputs,
    _state: &AnalysisStateMirror,
    element: &MatchedElement,
    suggestion: &LandmarkMatching,
    landmarks_updates: &mut HashMap<Uuid, Uuid>,
    updated_landmark_ids: &mut Vec<Uuid>,
) -> Result<Landmark, PpdcError> {
    // Check if we have a high-confidence match
    if let Some(ref candidate_id_str) = suggestion.candidate_id {
        if suggestion.confidence >= config.matching_confidence_threshold {
            let parent_landmark_id = Uuid::parse_str(candidate_id_str)?;

            // Check if we already created a child for this parent
            if let Some(&child_id) = landmarks_updates.get(&parent_landmark_id) {
                return Landmark::find(child_id, &context.pool);
            }

            // Create a child copy of the existing landmark
            let created_landmark = landmark::create_copy_child_and_return(
                parent_landmark_id,
                context.user_id,
                context.analysis_id,
                &context.pool,
            )?;

            landmarks_updates.insert(parent_landmark_id, created_landmark.id);
            updated_landmark_ids.push(parent_landmark_id);

            return Ok(created_landmark);
        }
    }

    // No high-confidence match - create new landmark via GPT
    let input = LandmarkCreationInput::from_matched(element, suggestion);
    create_landmark_via_gpt(context, &input).await
}

fn build_new_element_from_matched(
    element: &MatchedElement,
    interaction_date: Option<NaiveDateTime>,
    trace_id: Uuid,
    trace_mirror_id: Uuid,
    analysis_id: Uuid,
    user_id: Uuid,
) -> NewElement {
    let adjusted_interaction_date = apply_date_offset(interaction_date, element.date_offset);
    let evidences = if element.evidences.is_empty() {
        "[]".to_string()
    } else {
        element.evidences.join(", ")
    };

    NewElement::new(
        element.title.clone(),
        element.title.clone(),
        format!(
            "Evidences: {}\n\nExtractions: {:?}",
            evidences, element.extractions
        ),
        ElementType::TransactionOutput,
        element.verb.clone(),
        adjusted_interaction_date,
        trace_id,
        Some(trace_mirror_id),
        None,
        analysis_id,
        user_id,
    )
}

fn apply_date_offset(
    interaction_date: Option<NaiveDateTime>,
    date_offset: i32,
) -> Option<NaiveDateTime> {
    interaction_date.map(|interaction_date| {
        interaction_date
            .checked_add_signed(Duration::days(i64::from(date_offset)))
            .unwrap_or(interaction_date)
    })
}

#[cfg(test)]
mod tests {
    use super::build_new_element_from_matched;
    use crate::work_analyzer::elements_pipeline::extraction::ExtractionStatus;
    use crate::work_analyzer::elements_pipeline::matching::MatchedElement;
    use chrono::NaiveDate;
    use uuid::Uuid;

    #[test]
    fn build_new_element_from_matched_keeps_verb() {
        let matched_element = MatchedElement {
            temporary_id: "el-0".to_string(),
            title: "DONE - lire: Bullshit Jobs - Par David Graeber Sur travail".to_string(),
            verb: "DONE - lire".to_string(),
            status: ExtractionStatus::Done,
            date_offset: 0,
            evidences: vec!["Bullshit Jobs".to_string()],
            extractions: vec!["J'ai lu Bullshit Jobs".to_string()],
            landmark_suggestions: vec![],
        };

        let new_element = build_new_element_from_matched(
            &matched_element,
            None,
            Uuid::nil(),
            Uuid::nil(),
            Uuid::nil(),
            Uuid::nil(),
        );

        assert_eq!(new_element.verb, "DONE - lire");
    }

    #[test]
    fn build_new_element_from_matched_applies_date_offset_to_interaction_date() {
        let matched_element = MatchedElement {
            temporary_id: "el-1".to_string(),
            title: "DONE - lire: Bullshit Jobs - Par David Graeber Sur travail".to_string(),
            verb: "DONE - lire".to_string(),
            status: ExtractionStatus::Done,
            date_offset: -1,
            evidences: vec![],
            extractions: vec![],
            landmark_suggestions: vec![],
        };
        let interaction_date = NaiveDate::from_ymd_opt(2026, 2, 9)
            .expect("valid date")
            .and_hms_opt(9, 30, 0)
            .expect("valid time");

        let new_element = build_new_element_from_matched(
            &matched_element,
            Some(interaction_date),
            Uuid::nil(),
            Uuid::nil(),
            Uuid::nil(),
            Uuid::nil(),
        );

        let expected_date = NaiveDate::from_ymd_opt(2026, 2, 8)
            .expect("valid date")
            .and_hms_opt(9, 30, 0)
            .expect("valid time");
        assert_eq!(new_element.interaction_date, Some(expected_date));
    }
}
