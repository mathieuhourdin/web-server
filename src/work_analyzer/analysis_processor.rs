use crate::db::get_global_pool;
use crate::entities::{
    error::PpdcError,
    user::User,
};
use crate::entities_v2::{
    landscape_analysis::{
        LandscapeAnalysis,
    },
    trace::Trace,
    journal::Journal,
    landmark::Landmark,
};
use crate::work_analyzer::{
    trace_broker::{ResourceProcessor, ProcessorContext, LandmarkProcessor},
    high_level_analysis::get_high_level_analysis,
    trace_mirror,
};
use uuid::Uuid;
use crate::db::DbPool;

pub struct AnalysisProcessor {
    pub context: ProcessorContext,
}
impl AnalysisProcessor {
    pub fn new(context: ProcessorContext) -> Self {
        Self { context }
    }
    pub async fn process(self) -> Result<Vec<Landmark>, PpdcError> {
        let processors = vec![
            ResourceProcessor::new(),
        ];
        let trace = Trace::find_full_trace(self.context.trace.id, &self.context.pool)?;
        let journal_id = trace.journal_id.expect("Trace must have a journal");
        let journal = Journal::find_full(journal_id, &self.context.pool)?;
        let journal_subtitle = journal.subtitle;
        match journal_subtitle.as_str() {
            "journal" => {
                for processor in processors {
                    processor.process(&self.context).await?;
                }
            }
            "note" => {
                // Here we want to process the trace as a note, refering to a single resource.
                // First we need to find the resource referenced by the trace.
                // Then we match to an existing trace, or create a new one.
                // Then we split the trace into elements about themes.
                // I wonder : maybe i could use the same processor as for the landmark resource processor ?
                // At least for the matching and resource creation.
                // I just need to generate a new extraction prompt that will extract a single resource.
                // Then i match / create this resource ?
                // All elements should be linked to the same resource.
                // I just use a theme analysis with a default resource link for all elements.
                //processors.push(NoteProcessor::new());
            }
            _ => {}
        }
        Ok(vec![])
    }
}

pub async fn run_analysis_pipeline_for_landscapes(landscape_analysis_ids: Vec<Uuid>) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
    println!("work_analyzer::analysis_processor::run_analysis_pipeline_for_landscapes: Running analysis pipeline for landscapes: {:?}", landscape_analysis_ids);
    for landscape_analysis_id in landscape_analysis_ids {
        run_landscape_analysis(landscape_analysis_id).await?;
    }
    Ok(vec![])
}

pub async fn run_landscape_analysis(landscape_analysis_id: Uuid) -> Result<LandscapeAnalysis, PpdcError> {

    println!("work_analyzer::analysis_processor::run_landscape_analysis: Running analysis pipeline for landscape analysis id: {}", landscape_analysis_id);
    let pool = get_global_pool();

    let landscape_analysis: LandscapeAnalysis = LandscapeAnalysis::find_full_analysis(landscape_analysis_id, &pool)?;
    let previous_landscape_analysis_id = landscape_analysis.parent_analysis_id;
    let analyzed_trace_id = landscape_analysis.analyzed_trace_id;


    
    if let (Some(analyzed_trace_id), Some(previous_landscape_analysis_id)) = (analyzed_trace_id, previous_landscape_analysis_id) {
        return Ok(analysis_pipeline_with_trace(landscape_analysis_id, analyzed_trace_id, previous_landscape_analysis_id, &pool).await?);

    } else {
        return Ok(analysis_pipeline_without_trace(landscape_analysis_id, &pool).await?);
    }
}

/**
 * This function is used to run the analysis when there is no trace.
 * It happens for the root analysis, when we launch a new lens from the beginning of the user production.
 * We create a context from the user biography.
 */
pub async fn analysis_pipeline_without_trace(landscape_analysis_id: Uuid, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
    let landscape_analysis: LandscapeAnalysis = LandscapeAnalysis::find_full_analysis(landscape_analysis_id, &pool)?;
    let biography = User::find(&landscape_analysis.user_id, &pool)?.biography.unwrap_or_default();
    let previous_context = "Aucun contexte pour l'instant".to_string();
    let landscape_analysis = high_level_analysis_pipeline(&previous_context, landscape_analysis_id, &biography, pool).await?;
    Ok(landscape_analysis)
}

/**
 * This function is used to run the analysis when there is a trace.
 * It is the classical case.
 */
pub async fn analysis_pipeline_with_trace(landscape_analysis_id: Uuid, analyzed_trace_id: Uuid, previous_landscape_analysis_id: Uuid, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {

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
    let context = ProcessorContext {
        landmarks: previous_landscape_analysis_landmarks,
        trace: analyzed_trace.clone(),
        trace_mirror: trace_mirror.clone(),
        user_id: landscape_analysis.user_id,
        landscape_analysis_id,
        pool: pool.clone(),
    };
    let resource_processor = ResourceProcessor::new();
    let _new_landmarks = resource_processor.process(&context).await?;


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
    let landscape_analysis = high_level_analysis_pipeline(&previous_context, landscape_analysis_id, &full_trace_string, pool).await?;
    Ok(landscape_analysis)
}


pub async fn high_level_analysis_pipeline(previous_context: &String, landscape_analysis_id: Uuid, full_trace_string: &String, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
    let run_high_level_analysis = false;
    let landscape_analysis: LandscapeAnalysis = LandscapeAnalysis::find_full_analysis(landscape_analysis_id, &pool)?;

    if run_high_level_analysis {
    let new_high_level_analysis = get_high_level_analysis(
        previous_context, 
        full_trace_string)
        .await?;
        let mut landscape_analysis = landscape_analysis.clone();
        landscape_analysis.plain_text_state_summary = new_high_level_analysis;
        let landscape_analysis = landscape_analysis.update(&pool)?;
        return Ok(landscape_analysis);
    }
    Ok(landscape_analysis)
}