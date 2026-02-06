use crate::entities::{
    error::PpdcError,
    resource::MaturingState,
};
use crate::entities_v2::{
    landscape_analysis::{
        LandscapeAnalysis,
    },
    trace::Trace,
    journal::Journal,
    landmark::Landmark,
    trace_mirror::{self, TraceMirror},
    element::Element,
};
use crate::work_analyzer::{
    high_level_analysis,
    elements_pipeline::{self},
    mirror_pipeline,
};
use uuid::Uuid;
use crate::db::DbPool;



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
        self.run_high_level_analysis_pipeline(state).await?;

        let mut analysis = LandscapeAnalysis::find_full_analysis(self.context.analysis_id, &self.context.pool)?;
        analysis.processing_state = MaturingState::Finished;
        analysis = analysis.update(&self.context.pool)?;
        // return the new landscape
        Ok(analysis)
    }

    async fn create_initial_state(&self) -> Result<AnalysisStateInitial, PpdcError> {
        // When the analysis and landmarks will be splitted, this will be a creation instead of a find.
        let landscape = LandscapeAnalysis::find_full_analysis(self.context.analysis_id, &self.context.pool)?;
        Ok(AnalysisStateInitial {
            current_landscape: landscape,
        })
    }

    async fn run_mirror_pipeline(&self, state: AnalysisStateInitial) -> Result<AnalysisStateMirror, PpdcError> {
        let trace_header = mirror_pipeline::header::extract_mirror_header(&self.inputs.trace, self.context.analysis_id).await?;
        let trace_mirror = mirror_pipeline::header::create_trace_mirror(&self.inputs.trace, trace_header, &self.context).await?;
        let journal = Journal::find_full(self.inputs.trace.journal_id.unwrap(), &self.context.pool)?;
        let journal_subtitle = journal.subtitle;
        if journal_subtitle == "note" {
            let primary_resource_suggestion = mirror_pipeline::primary_resource::suggestion::extract(
                &self.inputs.trace,
                self.context.analysis_id,
            ).await?;
            let primary_resource_matched =
                mirror_pipeline::primary_resource::matching::run(
                    &self.context,
                    primary_resource_suggestion,
                    &self.inputs.previous_landscape_landmarks,
                ).await?;
            let trace_mirror_landmark_id: Uuid;
            if primary_resource_matched.candidate_id.is_some() {
                let primary_resource_landmark_id = primary_resource_matched.candidate_id.unwrap();
                trace_mirror_landmark_id = Uuid::parse_str(primary_resource_landmark_id.as_str())?;
            } else {
                let primary_resource_created =
                    mirror_pipeline::primary_resource::creation::run(primary_resource_matched, &self.context).await?;
                trace_mirror_landmark_id = primary_resource_created.id;
            }
            trace_mirror::persist::link_to_primary_resource(trace_mirror.id, trace_mirror_landmark_id, self.context.user_id, &self.context.pool)?;
        }
        Ok(AnalysisStateMirror {
            current_landscape: state.current_landscape.clone(),
            trace_mirror: trace_mirror,
        })
    }
    async fn run_trace_broker_pipeline(&self, state: AnalysisStateMirror) -> Result<AnalysisStateTraceBroker, PpdcError> {
        let extracted_elements =
            elements_pipeline::extraction::extract_elements(&self.config, &self.context, &self.inputs, &state).await?;
        let matched_elements =
            elements_pipeline::matching::match_elements(&self.config, &self.context, &self.inputs, &state, extracted_elements).await?;
        let created_elements =
            elements_pipeline::creation::create_elements(&self.config, &self.context, &self.inputs, &state, matched_elements).await?;
        let AnalysisStateMirror { current_landscape, trace_mirror } = state;
        Ok(AnalysisStateTraceBroker {
            current_landscape,
            current_trace_mirror: trace_mirror,
            current_landmarks: created_elements.created_landmarks,
            current_elements: created_elements.created_elements,
        })
    }

    async fn run_high_level_analysis_pipeline(&self, state: AnalysisStateTraceBroker) -> Result<AnalysisStateTraceBroker, PpdcError> {
        let AnalysisStateTraceBroker { current_landscape, current_trace_mirror, current_landmarks, current_elements } = state;
        let _high_level_analysis = high_level_analysis::high_level_analysis_pipeline(
            &current_landscape.plain_text_state_summary,
            self.context.analysis_id,
            &self.inputs.trace.content,
            &self.context.pool,
        ).await?;
        Ok(AnalysisStateTraceBroker {
            current_landscape,
            current_trace_mirror,
            current_landmarks,
            current_elements,
        })
    }
    pub fn setup(analysis_id: Uuid, trace_id: Uuid, previous_landscape_id: Uuid, pool: &DbPool) -> Result<AnalysisProcessor, PpdcError> {
        let trace = Trace::find_full_trace(trace_id, &pool)?;
        let user_id = trace.user_id;
        let previous_landscape = LandscapeAnalysis::find_full_analysis(previous_landscape_id, &pool)?;
        let previous_landscape_landmarks = previous_landscape.get_landmarks(None, &pool)?;
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
        Ok(processor)
    }
}
