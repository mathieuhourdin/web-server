use crate::entities::error::PpdcError;
use crate::entities_v2::{
    landmark::{self, Landmark, LandmarkType, NewLandmark},
    element::{link_to_landmark, Element, ElementType, NewElement},
    landscape_analysis,
};
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::trace_broker::traits::{ProcessorContext, NewLandmarkForExtractedElement};
use crate::work_analyzer::trace_broker::types::IdentityState;
use crate::work_analyzer::trace_broker::matching::{MatchedElements, MatchedElement, LandmarkMatching};
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
    pub context: ProcessorContext,
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
    input: &LandmarkCreationInput,
    context: &ProcessorContext,
) -> Result<Landmark, PpdcError> {
    let log_header = format!("analysis_id: {}", context.landscape_analysis_id);
    tracing::info!(
        target: "work_analyzer",
        "{} trace_broker_creation_llm_start landmark_type={:?} matching_key={}",
        log_header,
        input.landmark_type,
        input.matching_key
    );
    
    let user_prompt = serde_json::to_string_pretty(input)?;
    let system_prompt = get_system_prompt_for_type(input.landmark_type);
    let schema = get_schema_for_type(input.landmark_type);
    
    let gpt_config = GptRequestConfig::new(
        "gpt-4.1-nano".to_string(),
        &system_prompt,
        &user_prompt,
        Some(schema),
    ).with_log_header(log_header.as_str());
    
    // Call GPT and deserialize based on type
    let new_landmark: NewLandmark = match input.landmark_type {
        LandmarkType::Resource => {
            let result: NewResourceLandmark = gpt_config.execute().await?;
            result.to_new_landmark(context.landscape_analysis_id, context.user_id, None)
        }
        LandmarkType::Theme => {
            let result: NewThemeLandmark = gpt_config.execute().await?;
            result.to_new_landmark(context.landscape_analysis_id, context.user_id, None)
        }
        LandmarkType::Author => {
            let result: NewAuthorLandmark = gpt_config.execute().await?;
            result.to_new_landmark(context.landscape_analysis_id, context.user_id, None)
        }
    };
    
    let created_landmark = new_landmark.create(&context.pool)?;
    tracing::info!(
        target: "work_analyzer",
        "{} trace_broker_creation_llm_complete landmark_id={} landmark_type={:?}",
        log_header,
        created_landmark.id,
        created_landmark.landmark_type
    );
    Ok(created_landmark)
}

// ============================================================================
// Main Creation Logic
// ============================================================================

/// Confidence threshold for linking to existing landmarks
const CONFIDENCE_THRESHOLD: f32 = 0.4;

impl MatchedElements {
    /// Process matched elements to create or link landmarks and create elements.
    /// 
    /// For each element's landmark_suggestion:
    /// - If candidate_id exists and confidence >= threshold: create a child copy of the existing landmark
    /// - Otherwise: create a new landmark via GPT
    /// 
    /// Then create Element records linking traces to landmarks.
    pub async fn run_creation(self) -> Result<CreatedElements, PpdcError> {
        let log_header = format!("analysis_id: {}", self.context.landscape_analysis_id);
        tracing::info!(
            target: "work_analyzer",
            "{} trace_broker_creation_start elements={}",
            log_header,
            self.elements.len()
        );
        
        let mut created_landmarks: Vec<Landmark> = vec![];
        let mut created_elements: Vec<Element> = vec![];
        
        // Track which parent landmarks were updated (to avoid duplicate refs)
        let mut updated_landmark_ids: Vec<Uuid> = vec![];
        // Map from parent landmark ID to created child landmark ID
        let mut landmarks_updates: HashMap<Uuid, Uuid> = HashMap::new();
        
        for element in &self.elements {
            let new_element = NewElement::new(
                element.title.clone(),
                element.title.clone(),
                format!("{}\n\nExtractions: {:?}", element.evidence, element.extractions),
                ElementType::Event,
                self.context.trace.id,
                Some(self.context.trace_mirror.id),
                None,
                self.context.landscape_analysis_id,
                self.context.user_id,
            );
            let created_element = new_element.create(&self.context.pool)?;
            created_elements.push(created_element.clone());

            // Process each landmark suggestion for this element
            for suggestion in &element.landmark_suggestions {
                let created_landmark = self.process_suggestion(
                    element,
                    suggestion,
                    &mut landmarks_updates,
                    &mut updated_landmark_ids,
                ).await?;
                
                link_to_landmark(
                    created_element.id,
                    created_landmark.id,
                    self.context.user_id,
                    &self.context.pool,
                )?;
                created_landmarks.push(created_landmark);
            }
        }
        
        // Add landmark refs for landmarks that were not updated
        for landmark in &self.context.landmarks {
            if !updated_landmark_ids.contains(&landmark.id) {
                landscape_analysis::add_landmark_ref(
                    self.context.landscape_analysis_id,
                    landmark.id,
                    self.context.user_id,
                    &self.context.pool,
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
            context: self.context,
            created_landmarks,
            created_elements,
        })
    }
    
    /// Process a single landmark suggestion
    async fn process_suggestion(
        &self,
        element: &MatchedElement,
        suggestion: &LandmarkMatching,
        landmarks_updates: &mut HashMap<Uuid, Uuid>,
        updated_landmark_ids: &mut Vec<Uuid>,
    ) -> Result<Landmark, PpdcError> {
        // Check if we have a high-confidence match
        if let Some(ref candidate_id_str) = suggestion.candidate_id {
            if suggestion.confidence >= CONFIDENCE_THRESHOLD {
                let parent_landmark_id = Uuid::parse_str(candidate_id_str)?;
                
                // Check if we already created a child for this parent
                if let Some(&child_id) = landmarks_updates.get(&parent_landmark_id) {
                    return Landmark::find(child_id, &self.context.pool);
                }
                
                // Create a child copy of the existing landmark
                let created_landmark = landmark::create_copy_child_and_return(
                    parent_landmark_id,
                    self.context.user_id,
                    self.context.landscape_analysis_id,
                    &self.context.pool,
                )?;
                
                landmarks_updates.insert(parent_landmark_id, created_landmark.id);
                updated_landmark_ids.push(parent_landmark_id);
                
                return Ok(created_landmark);
            }
        }
        
        // No high-confidence match - create new landmark via GPT
        let input = LandmarkCreationInput::from_matched(element, suggestion);
        create_landmark_via_gpt(&input, &self.context).await
    }
}
