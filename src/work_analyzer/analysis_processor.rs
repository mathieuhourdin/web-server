use crate::db::get_global_pool;
use crate::entities::{
    error::PpdcError,
    user::User,
    resource::MaturingState,
};
use crate::entities_v2::{
    landscape_analysis::{
        LandscapeAnalysis,
    },
    trace::Trace,
    journal::Journal,
    landmark::Landmark,
    trace_mirror::TraceMirror,
    element::Element,
};
use crate::work_analyzer::{
    trace_broker::{creation, extraction, matching, ProcessorContext},
    high_level_analysis,
    trace_mirror,
};
use uuid::Uuid;
use crate::db::DbPool;
use crate::environment::get_env;


// We want to first build a context for the analysis. Then all the pipeline should come out of this context (is it a first trace etc..)
// The 

// Technically this is the analysis function that helps create a new landscape from the previous one and a new trace.
// Pipeline without trace is a special case that should not be handled with the general pipeline.
// In a really denormalized implementation, we should have a separated analysis and landscape.
// The anaysis produces a landscape from a previous landscape and a new trace.
// When we run the analysis, we have a already existing analysis, but the landscape is not yet created.
// I think the lens should hold the fact that we want to run multiple analysis from one point to the other.
// However the anaysis should be created one after the other. Or with a "depend on :" relation between analyses.
// While the current_trace and target_trace of a lens are different and lens is not in failed state, we should launch analysis runs to make the two match.
// Then the analysis should be created and run one after the other.
// I record the fact that there should be a special pipeline for the first analysis, which should create landscape from the user biography.

// here we assume that the analysis is related to a trace.

pub struct AnalysisConfig {
    pub model: String,
    pub matching_confidence_threshold: f32,
    pub feature_flag_run_high_level_analysis: bool,
}
pub struct AnalysisContext {
    pub analysis_id: Uuid,
    pub user_id: Uuid,
    pub pool: DbPool,
}

impl AnalysisContext {
    pub fn log_header(&self) -> String {
        format!("analysis_id: {}", self.analysis_id)
    }
}

pub struct AnalysisInputs {
    pub trace: Trace,
    pub previous_landscape: LandscapeAnalysis,
    pub previous_landscape_landmarks: Vec<Landmark>,
}

#[derive(Clone)]
pub struct AnalysisStateInitial {
    pub current_landscape: LandscapeAnalysis,
}

pub struct AnalysisStateMirror {
    pub current_landscape: LandscapeAnalysis,
    pub trace_mirror: TraceMirror,
}

impl AnalysisStateMirror {
    pub fn to_processor_context(
        &self,
        context: &AnalysisContext,
        inputs: &AnalysisInputs,
    ) -> ProcessorContext {
        ProcessorContext {
            landmarks: inputs.previous_landscape_landmarks.clone(),
            trace: inputs.trace.clone(),
            trace_mirror: self.trace_mirror.clone(),
            user_id: context.user_id,
            landscape_analysis_id: context.analysis_id,
            pool: context.pool.clone(),
        }
    }
}

pub struct AnalysisStateTraceBroker {
    pub current_landscape: LandscapeAnalysis,
    pub current_trace_mirror: TraceMirror,
    pub current_landmarks: Vec<Landmark>,
    pub current_elements: Vec<Element>,
}

pub struct AnalysisProcessor {
    pub context: AnalysisContext,
    pub config: AnalysisConfig,
    pub inputs: AnalysisInputs,
}

impl AnalysisProcessor {
    pub fn new(context: AnalysisContext, config: AnalysisConfig, inputs: AnalysisInputs) -> Self {
        Self { context, config, inputs }
    }
    pub async fn process(self) -> Result<LandscapeAnalysis, PpdcError> {

        let state = self.create_initial_state().await?;

        // run mirror pipeline
        let state = self.run_mirror_pipeline(state).await?;

        // run trace broker pipeline
        let state = self.run_trace_broker_pipeline(state).await?;

        // run high level analysis pipeline
        let state = self.run_high_level_analysis_pipeline(state).await?;

        let mut analysis = LandscapeAnalysis::find_full_analysis(self.context.analysis_id, &self.context.pool)?;
        analysis.processing_state = MaturingState::Finished;
        analysis = analysis.update(&self.context.pool)?;
        // return the new landscape
        Ok(state.current_landscape)
    }

    async fn create_initial_state(&self) -> Result<AnalysisStateInitial, PpdcError> {
        // When the analysis and landmarks will be splitted, this will be a creation instead of a find.
        let landscape = LandscapeAnalysis::find_full_analysis(self.context.analysis_id, &self.context.pool)?;
        Ok(AnalysisStateInitial {
            current_landscape: landscape,
        })
    }

    async fn run_mirror_pipeline(&self, state: AnalysisStateInitial) -> Result<AnalysisStateMirror, PpdcError> {
        let trace_mirror = trace_mirror::pipeline::run(
            &self.config,
            &self.context,
            &self.inputs,
            &state,
        ).await?;
        let AnalysisStateInitial { current_landscape } = state;
        Ok(AnalysisStateMirror {
            current_landscape,
            trace_mirror,
        })
    }

    async fn run_trace_broker_pipeline(&self, state: AnalysisStateMirror) -> Result<AnalysisStateTraceBroker, PpdcError> {
        let extracted_elements =
            extraction::extract_elements(&self.config, &self.context, &self.inputs, &state).await?;
        let matched_elements =
            matching::match_elements(&self.config, &self.context, &self.inputs, extracted_elements).await?;
        let created_elements =
            creation::create_elements(&self.config, &self.context, &self.inputs, matched_elements).await?;
        let AnalysisStateMirror { current_landscape, trace_mirror } = state;
        Ok(AnalysisStateTraceBroker {
            current_landscape,
            current_trace_mirror: trace_mirror,
            current_landmarks: created_elements.created_landmarks,
            current_elements: created_elements.created_elements,
        })
    }

    async fn run_high_level_analysis_pipeline(&self, state: AnalysisStateTraceBroker) -> Result<AnalysisStateTraceBroker, PpdcError> {
        if !self.config.feature_flag_run_high_level_analysis {
            return Ok(state);
        }
        let log_header = self.context.log_header();
        let _high_level_analysis = high_level_analysis::high_level_analysis_pipeline(
            &self.inputs.previous_landscape.plain_text_state_summary,
            self.context.analysis_id,
            &self.inputs.trace.content,
            &self.context.pool,
            &log_header.as_str(),
        );
        Ok(state)
    }
}

pub async fn create_next_landscape(analysis_id: Uuid, trace_id: Uuid, previous_landscape_id: Uuid, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    let user_id = trace.user_id;
    let previous_landscape = LandscapeAnalysis::find_full_analysis(previous_landscape_id, &pool)?;
    let previous_landscape_landmarks = previous_landscape.get_landmarks(&pool)?;
    let analysis_config = AnalysisConfig {
        model: "gpt-4.1-mini".to_string(),
        matching_confidence_threshold: 0.3,
        feature_flag_run_high_level_analysis: false,
    };
    let context = AnalysisContext {
        analysis_id,
        user_id,
        pool: pool.clone()
    };
    let inputs = AnalysisInputs {
        trace,
        previous_landscape,
        previous_landscape_landmarks,
    };
    let processor = AnalysisProcessor::new(context, analysis_config, inputs);
    processor.process().await
}




/*pub async fn run_landscape_analysis(landscape_analysis_id: Uuid) -> Result<LandscapeAnalysis, PpdcError> {

    let log_header = format!("analysis_id: {}", landscape_analysis_id);
    tracing::info!(
        target: "work_analyzer",
        "{} analysis_run_start",
        log_header
    );
    let pool = get_global_pool();

    let landscape_analysis: LandscapeAnalysis = LandscapeAnalysis::find_full_analysis(landscape_analysis_id, &pool)?;
    let previous_landscape_analysis_id = landscape_analysis.parent_analysis_id;
    let analyzed_trace_id = landscape_analysis.analyzed_trace_id;

    tracing::info!(
        target: "work_analyzer",
        "{} analysis_context user_id={} analyzed_trace_id={:?} parent_analysis_id={:?}",
        log_header,
        landscape_analysis.user_id,
        analyzed_trace_id,
        previous_landscape_analysis_id
    );
    
    if let (Some(analyzed_trace_id), Some(previous_landscape_analysis_id)) = (analyzed_trace_id, previous_landscape_analysis_id) {
        return Ok(analysis_pipeline_with_trace(landscape_analysis_id, analyzed_trace_id, previous_landscape_analysis_id, &pool).await?);

    } else {
        return Ok(analysis_pipeline_without_trace(landscape_analysis_id, &pool).await?);
    }
}*/

/*
 * This function is used to run the analysis when there is no trace.
 * It happens for the root analysis, when we launch a new lens from the beginning of the user production.
 * We create a context from the user biography.
 */
/*pub async fn analysis_pipeline_without_trace(landscape_analysis_id: Uuid, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
    let log_header = format!("analysis_id: {}", landscape_analysis_id);
    let landscape_analysis: LandscapeAnalysis = LandscapeAnalysis::find_full_analysis(landscape_analysis_id, &pool)?;
    let biography = User::find(&landscape_analysis.user_id, &pool)?.biography.unwrap_or_default();
    let previous_context = "Aucun contexte pour l'instant".to_string();
    let landscape_analysis = high_level_analysis::high_level_analysis_pipeline(&previous_context, landscape_analysis_id, &biography, pool, &log_header).await?;
    Ok(landscape_analysis)
}*/

/*
 This function is used to run the analysis when there is a trace.
 It is the classical case.
 */
/*pub async fn analysis_pipeline_with_trace(landscape_analysis_id: Uuid, analyzed_trace_id: Uuid, previous_landscape_analysis_id: Uuid, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {

    let log_header = format!("analysis_id: {}", landscape_analysis_id);
    let landscape_analysis: LandscapeAnalysis = LandscapeAnalysis::find_full_analysis(landscape_analysis_id, &pool)?;
      
    // build the context of the previous landscape analysis
    let previous_landscape_analysis: LandscapeAnalysis = LandscapeAnalysis::find_full_analysis(previous_landscape_analysis_id, &pool)?;
    let previous_landscape_analysis_landmarks = previous_landscape_analysis.get_landmarks(&pool)?;

    let analyzed_trace = Trace::find_full_trace(analyzed_trace_id, &pool)?;


    // First stage of processing 
    // Create the trace mirror element
    let trace_mirror = trace_mirror::pipeline::run(
        &analyzed_trace, 
        landscape_analysis.user_id, 
        landscape_analysis_id,
        &previous_landscape_analysis_landmarks,
         &pool)
        .await?;


    // Second stage of processing
    // Run the landmark analysis pipeline
    tracing::info!(
        target: "work_analyzer",
        "{} trace_broker_pipeline_start",
        log_header
    );
    let context = ProcessorContext {
        landmarks: previous_landscape_analysis_landmarks,
        trace: analyzed_trace.clone(),
        trace_mirror: trace_mirror.clone(),
        user_id: landscape_analysis.user_id,
        landscape_analysis_id,
        pool: pool.clone(),
    };
    let extractable = Extractable::from(context);
    extractable
        .extract()
        .await?
        .run_matching()
        .await?
        .run_creation()
        .await?;
    tracing::info!(
        target: "work_analyzer",
        "{} trace_broker_pipeline_complete",
        log_header
    );
    // let resource_processor = ResourceProcessor::new();
    // let _new_landmarks = resource_processor.process(&context).await?;


    // Third stage of processing
    // Run the high level analysis pipeline
    let full_trace_string = format!("
        Titre : {}\n
        Sous titre : {}\n
        Contenu : {}\n",
        analyzed_trace.title,
        analyzed_trace.subtitle,
        analyzed_trace.content
    );
    let previous_context = previous_landscape_analysis.plain_text_state_summary;
    let landscape_analysis =
        high_level_analysis::high_level_analysis_pipeline(&previous_context, landscape_analysis_id, &full_trace_string, pool, &log_header).await?;
    let mut landscape_analysis = landscape_analysis;
    landscape_analysis.processing_state = MaturingState::Finished;
    landscape_analysis = landscape_analysis.update(&pool)?;
    Ok(landscape_analysis)
}*/
