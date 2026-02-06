use crate::entities::error::PpdcError;
use crate::entities_v2::{
    landmark::{self, Landmark, LandmarkType, NewLandmark},
    element::{link_to_landmark, Element, ElementType, NewElement},
    landscape_analysis,
};
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::elements_pipeline::traits::NewLandmarkForExtractedElement;
use crate::work_analyzer::elements_pipeline::types::IdentityState;
use crate::work_analyzer::elements_pipeline::matching::{MatchedElements, MatchedElement, LandmarkMatching};
use crate::work_analyzer::analysis_processor::{AnalysisConfig, AnalysisContext, AnalysisInputs, AnalysisStateMirror};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;

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
    /// From parent MatchedElement - evidence/citation from the trace
    pub evidence: String,
    /// From parent MatchedElement - extracted insights
    pub extractions: Vec<String>,
}

impl LandmarkCreationInput {
    pub fn from_matched(element: &MatchedElement, suggestion: &LandmarkMatching) -> Self {
        Self {
            matching_key: suggestion.matching_key.clone(),
            landmark_type: suggestion.landmark_type,
            element_title: element.title.clone(),
            evidence: element.evidence.clone(),
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
        LandmarkType::Theme
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
        LandmarkType::Author
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
        LandmarkType::Resource => include_str!("prompts/landmark_resource/creation/system.md").to_string(),
        LandmarkType::Theme => include_str!("prompts/landmark_theme/creation/system.md").to_string(),
        LandmarkType::Author => include_str!("prompts/landmark_author/creation/system.md").to_string(),
    }
}

fn get_schema_for_type(landmark_type: LandmarkType) -> serde_json::Value {
    match landmark_type {
        LandmarkType::Resource => serde_json::from_str(include_str!("prompts/landmark_resource/creation/schema.json")).unwrap(),
        LandmarkType::Theme => serde_json::from_str(include_str!("prompts/landmark_theme/creation/schema.json")).unwrap(),
        LandmarkType::Author => serde_json::from_str(include_str!("prompts/landmark_author/creation/schema.json")).unwrap(),
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
    let log_header = context.log_header();
    let user_prompt = serde_json::to_string_pretty(input)?;
    let system_prompt = get_system_prompt_for_type(input.landmark_type);
    let schema = get_schema_for_type(input.landmark_type);
    let display_name = match input.landmark_type {
        LandmarkType::Resource => "Elements / Creation / Resource",
        LandmarkType::Theme => "Elements / Creation / Theme",
        LandmarkType::Author => "Elements / Creation / Author",
    };
    
    let gpt_config = GptRequestConfig::new(
        "gpt-4.1-nano".to_string(),
        &system_prompt,
        &user_prompt,
        Some(schema),
        Some(context.analysis_id),
    )
    .with_log_header(log_header.as_str())
    .with_display_name(display_name);
    
    // Call GPT and deserialize based on type
    let new_landmark: NewLandmark = match input.landmark_type {
        LandmarkType::Resource => {
            let result: NewResourceLandmark = gpt_config.execute().await?;
            result.to_new_landmark(context.analysis_id, context.user_id, None)
        }
        LandmarkType::Theme => {
            let result: NewThemeLandmark = gpt_config.execute().await?;
            result.to_new_landmark(context.analysis_id, context.user_id, None)
        }
        LandmarkType::Author => {
            let result: NewAuthorLandmark = gpt_config.execute().await?;
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
        let new_element = NewElement::new(
            element.title.clone(),
            element.title.clone(),
            format!("{}\n\nExtractions: {:?}", element.evidence, element.extractions),
            ElementType::Event,
            inputs.trace.id,
            Some(state.trace_mirror.id),
            None,
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
            ).await?;
            
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
