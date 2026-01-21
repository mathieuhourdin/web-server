use crate::db::get_global_pool;
use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::{Interaction, InteractionWithResource},
    resource::{maturing_state::MaturingState, Resource},
    resource_relation::NewResourceRelation,
};
use crate::entities_v2::{
    landscape_analysis::{
        self, 
        LandscapeAnalysis,
    },
    landmark::Landmark,
};
use crate::work_analyzer::{
    trace_broker::{ResourceProcessor, ProcessorContext, LandmarkProcessor},
    high_level_analysis::get_high_level_analysis,
    high_level_analysis::HighLevelAnalysis,
};
use chrono::NaiveDate;
use uuid::Uuid;

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
    let landscape_analysis = LandscapeAnalysis::find_full_analysis(landscape_analysis_id, &pool)?;
    let previous_landscape_analysis_id = landscape_analysis.parent_analysis_id.expect("Should have a parent analysis id at this stage");
    let previous_landscape_analysis = LandscapeAnalysis::find_full_analysis(previous_landscape_analysis_id, &pool)?;
    let previous_landscape_analysis_landmarks = previous_landscape_analysis.get_landmarks(&pool)?;
    let current_landscape_analysis_trace = Resource::find(landscape_analysis.analyzed_trace_id.expect("Should have a trace id at this stage"), &pool)?;
    let context = ProcessorContext {
        landmarks: previous_landscape_analysis_landmarks,
        trace: current_landscape_analysis_trace,
        user_id: landscape_analysis.user_id,
        analysis_resource_id: landscape_analysis_id,
        pool: pool.clone(),
    };
    let resource_processor = ResourceProcessor::new();
    let _new_landmarks = resource_processor.process(&context).await?;
    Ok(landscape_analysis)
}

pub async fn run_analysis_pipeline(landscape_analysis_id: Uuid) -> Result<LandscapeAnalysis, PpdcError> {
    println!("work_analyzer::analysis_processor::run_analysis_pipeline: Running analysis pipeline for analysis id: {}", landscape_analysis_id);
    let pool = get_global_pool();

    let landscape_analysis = LandscapeAnalysis::find_full_analysis(landscape_analysis_id, &pool)?;

    let user_id = landscape_analysis.user_id;
    let date = landscape_analysis.interaction_date;


    let last_analysis = landscape_analysis::find_last_analysis_resource(user_id, &pool)?;
    let last_analysis_option_id = last_analysis.as_ref().map(|last_analysis| last_analysis.resource.id).clone();

    let high_level_analysis = process_analysis(user_id, date.unwrap().date(), &landscape_analysis_id, last_analysis_option_id, &pool)
        .await
        .map_err(|e| {
            let _ = landscape_analysis::delete_leaf_and_cleanup(landscape_analysis_id, &pool);
            println!("Error: {:?}", e);
            e
        })?;
    let mut landscape_analysis = landscape_analysis;
    landscape_analysis.title = high_level_analysis.title;
    landscape_analysis.subtitle = high_level_analysis.subtitle;
    landscape_analysis.plain_text_state_summary = high_level_analysis.content;
    landscape_analysis.processing_state = MaturingState::Finished;
    let landscape_analysis = landscape_analysis.update(&pool)?;

    match last_analysis {
        Some(last_analysis) => {
            let mut last_analysis_resource = last_analysis.resource;
            last_analysis_resource.maturing_state = MaturingState::Trashed;
            let last_analysis_resource = last_analysis_resource.update(&pool)?;
            let mut new_parent_relation =
                NewResourceRelation::new(landscape_analysis.id, last_analysis_resource.id);
            new_parent_relation.user_id = Some(user_id);
            new_parent_relation.relation_type = Some("prnt".to_string());
            new_parent_relation.create(pool)?;
            Ok(landscape_analysis)
        }
        None => {
            Ok(landscape_analysis)
        }
    }
}

pub async fn process_analysis(
    user_id: Uuid,
    date: NaiveDate,
    analysis_resource_id: &Uuid,
    last_analysis_id_option: Option<Uuid>,
    pool: &DbPool,
) -> Result<HighLevelAnalysis, PpdcError> {
    println!("work_analyzer::analysis_processor::process_analysis: Processing analysis for user id: {}, date: {}, analysis resource id: {}, last analysis id option: {:?}", user_id, date, analysis_resource_id, last_analysis_id_option);
    let interactions =
        Interaction::find_all_draft_traces_for_user_before_date(user_id, date, pool)?;

    // if no new traces found, we dont process the analysis
    if interactions.is_empty() {
        return Ok(HighLevelAnalysis {
            title: "Période sans activité".to_string(),
            subtitle: "Aucune activité n'a été enregistrée pendant cette période".to_string(),
            content: String::new(),
        });
    }
    println!("Interactions: {:?}", interactions);

    let analysis_result = process_traces(
        &interactions,
        analysis_resource_id,
        last_analysis_id_option,
        user_id,
        pool,
    )
    .await?;

    for interaction in interactions {
        let mut new_resource_relation =
            NewResourceRelation::new(interaction.resource.id, analysis_resource_id.clone());
        new_resource_relation.user_id = Some(user_id);
        new_resource_relation.relation_type = Some("srcs".to_string());
        new_resource_relation.create(pool)?;
        let mut trace = interaction.resource;
        trace.maturing_state = MaturingState::Finished;
        trace.update(pool)?;
    }

    Ok(analysis_result)
}

pub async fn process_traces(
    traces: &Vec<InteractionWithResource>,
    analysis_resource_id: &Uuid,
    last_analysis_id_option: Option<Uuid>,
    user_id: Uuid,
    pool: &DbPool,
) -> Result<HighLevelAnalysis, PpdcError> {
    println!("work_analyzer::analysis_processor::process_traces: Processing traces for user id: {}, analysis resource id: {}, last analysis id option: {:?}", user_id, analysis_resource_id, last_analysis_id_option);
    // here we process the traces using the trace broker to identify simple elements that will be used to create analysis landmarks
    let mut simple_elements: Vec<Resource> = vec![];

    for trace in traces {
        let analysis_landmarks = Landmark::find_all_up_to_date_for_user(user_id, pool)?;
        println!("Processing trace: {:?}", trace.resource);
        let processed_elements = process_trace(
            &trace.resource,
            user_id,
            &analysis_landmarks,
            analysis_resource_id.clone(),
            pool,
        )
        .await?;
        simple_elements.extend(processed_elements);
    }

    // we get the high level analysis from the gpt
    let traces_resources = traces
        .iter()
        .map(|interaction| interaction.resource.clone())
        .collect::<Vec<_>>();
    let high_level_analysis = get_high_level_analysis(&traces_resources).await?;

    Ok(high_level_analysis)
}

pub async fn process_trace(
    trace: &Resource,
    user_id: Uuid,
    landmarks: &Vec<Landmark>,
    analysis_resource_id: Uuid,
    pool: &DbPool,
) -> Result<Vec<Resource>, PpdcError> {
    let resource_processor = ResourceProcessor::new();
    let context = ProcessorContext {
        landmarks: landmarks.clone(),
        trace: trace.clone(),
        user_id: user_id,
        analysis_resource_id: analysis_resource_id,
        pool: pool.clone(),
    };
    let new_landmarks = resource_processor.process(&context).await?;
    let new_resources = new_landmarks.into_iter().map(|landmark| Landmark::to_resource(landmark.clone())).collect::<Vec<Resource>>();
    Ok(new_resources)
}