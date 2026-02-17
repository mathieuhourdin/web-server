use uuid::Uuid;
use crate::entities::{error::PpdcError, resource::MaturingState};
use crate::entities_v2::{
    landscape_analysis::{LandscapeAnalysis, NewLandscapeAnalysis},
    lens::Lens,
    trace::Trace
};
use crate::db::{DbPool, get_global_pool};
use crate::work_analyzer::analysis_processor;
use chrono::Utc;

/*pub async fn run_analysis_pipeline_for_landscapes(landscape_analysis_ids: Vec<Uuid>) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
    println!("run_analysis_pipeline_for_landscapes: {:?}", landscape_analysis_ids);
    tracing::info!(
        target: "work_analyzer",
        "analysis_id: batch run_analysis_pipeline_for_landscapes ids={:?}",
        landscape_analysis_ids
    );
    for landscape_analysis_id in landscape_analysis_ids {
        run_landscape_analysis(landscape_analysis_id).await?;
    }
    println!("run_analysis_pipeline_for_landscapes complete");
    Ok(vec![])
}*/

pub async fn run_lens(lens_id: Uuid) -> Result<Lens, PpdcError> {
    println!("run_lens: {:?}", lens_id);
    let pool = get_global_pool();
    let mut lens = Lens::find_full_lens(lens_id, &pool.clone())?;
    while let Some(new_lens) = run_lens_step(lens.id, &pool.clone()).await? {
        lens = new_lens;
    }
    lens = lens.set_processing_state(MaturingState::Finished, &pool)?;
    Ok(lens)
}

pub async fn run_lens_step(lens_id: Uuid, pool: &DbPool) -> Result<Option<Lens>, PpdcError> {
    println!("run_lens_step: {:?}", lens_id);
    let lens = Lens::find_full_lens(lens_id, &pool)?;
    println!("lens: {:?}", lens);
    let current_landscape_id = lens.current_landscape_id;
    /*if current_landscape_id.is_none() {
        println!("Creating first landscape");
        let initial_landscape = create_first_landscape(lens.user_id.expect("User id is required"), &pool).await?;
        let lens = lens.update_current_landscape(initial_landscape.id, &pool)?;
        println!("Lens updated with first landscape: {:?}", lens);
        return Ok(Some(lens));
    } */
    //let current_landscape_id = current_landscape_id.expect("Current landscape id is required");
    //let current_landscape = LandscapeAnalysis::find_full_analysis(current_landscape_id, &pool)?;
    //let current_trace_id = current_landscape.analyzed_trace_id;

    // check the replay case first.
    if current_landscape_id.is_some() {
        let current_landscape = LandscapeAnalysis::find_full_analysis(current_landscape_id.unwrap(), &pool)?;
        if current_landscape.analyzed_trace_id == Some(lens.target_trace_id) {
            // The lens is up to date, no need to run the pipeline.
            // Except if this is a replay, in which case we should run the pipeline but keep the current landscape as replayed from.
            if lens.processing_state == MaturingState::Replay {
                let new_analysis = NewLandscapeAnalysis::new(
                    format!("Replay de l'Analyse de la trace {}", current_landscape.analyzed_trace_id.unwrap()),
                    String::new(),
                    String::new(),
                    lens.user_id.expect("User id is required"),
                    Utc::now().naive_utc(),
                    current_landscape.parent_analysis_id,
                    current_landscape.analyzed_trace_id,
                    Some(current_landscape.id),
                ).create(&pool)?;
                let _lens = lens.update_current_landscape(new_analysis.id, &pool)?;
                let processor = analysis_processor::AnalysisProcessor::setup(
                    new_analysis.id,
                    current_landscape.analyzed_trace_id.unwrap(),
                    current_landscape_id,
                    &pool
                )?;
                let _new_landscape = processor.process().await?;
            } 
            // This should stop the pipeline if we have reached the target trace.
            return Ok(None);
        }
    }
    // The lens is not up to date, run the pipeline.
    let next_trace: Option<Trace>;
    let title: String;
    if current_landscape_id.is_none() {
        next_trace = Trace::get_first(lens.user_id.expect("User id is required"), &pool)?;
        title = String::from("Premiere analyse ");
    } else {
        let current_landscape = LandscapeAnalysis::find_full_analysis(current_landscape_id.unwrap(), &pool)?;
        next_trace = Trace::get_next(lens.user_id.expect("User id is required"), current_landscape.analyzed_trace_id.unwrap(), &pool)?;
        title = String::from("Analyse de la trace suivante ");
    }
    if next_trace.is_none() {
        // It should not happen, but if it does, we should stop the pipeline.
        return Ok(None);
    }
    let next_trace = next_trace.unwrap();
    let analysis_title = format!("{} {}", title, next_trace.interaction_date.unwrap().format("%Y-%m-%d").to_string());
    let new_analysis = NewLandscapeAnalysis::new(
        analysis_title,
        String::new(),
        String::new(),
        lens.user_id.expect("User id is required"),
        Utc::now().naive_utc(),
        current_landscape_id,
        Some(next_trace.id),
        None,
    ).create(&pool)?;
    let lens = lens.update_current_landscape(new_analysis.id, &pool)?;
    let processor = analysis_processor::AnalysisProcessor::setup(
        new_analysis.id,
        next_trace.id,
        current_landscape_id,
        &pool
    )?;
    let _new_landscape = processor.process().await?;
    Ok(Some(lens))
}

pub async fn create_first_landscape(user_id: Uuid, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {

    let initial_landscape = NewLandscapeAnalysis::new(
        "Analyse de la biographie".to_string(),
        "Analyse de la biographie".to_string(),
        "Analyse de la biographie".to_string(),
        user_id,
        Utc::now().naive_utc(),
        None,
        None,
        None,
    ).create(&pool)?;
    Ok(initial_landscape)
}
    
