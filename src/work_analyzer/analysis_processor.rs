use crate::db::get_global_pool;
use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::{Interaction, InteractionWithResource},
    resource::{maturing_state::MaturingState, Resource},
    resource_relation::NewResourceRelation,
};
use crate::entities_v2::{
    analysis::{delete_analysis_resources_and_clean_graph, find_last_analysis_resource},
    landmark::Landmark,
};
use crate::work_analyzer::{
    trace_broker::{ResourceProcessor, ProcessorContext, LandmarkProcessor},
    high_level_analysis::get_high_level_analysis,
    high_level_analysis::HighLevelAnalysis,
};
use chrono::NaiveDate;
use uuid::Uuid;

pub async fn run_analysis_pipeline(analysis_id: Uuid) -> Result<Resource, PpdcError> {
    println!("work_analyzer::analysis_processor::run_analysis_pipeline: Running analysis pipeline for analysis id: {}", analysis_id);
    let pool = get_global_pool();

    let analysis = Resource::find(analysis_id, &pool)?;

    let analysis_interaction = &analysis.find_resource_author_interaction(&pool)?;
    let user_id = analysis_interaction.interaction_user_id;
    let date = analysis_interaction
        .interaction_date
        .date();

    let last_analysis = find_last_analysis_resource(analysis_interaction.interaction_user_id, &pool)?;
    let last_analysis_option_id = last_analysis.as_ref().map(|last_analysis| last_analysis.resource.id).clone();

    let high_level_analysis = process_analysis(user_id, date, &analysis.id, last_analysis_option_id, &pool)
        .await
        .map_err(|e| {
            let _ = delete_analysis_resources_and_clean_graph(analysis_id, &pool);
            println!("Error: {:?}", e);
            e
        })?;
    let mut analysis = analysis;
    analysis.title = high_level_analysis.title;
    analysis.subtitle = high_level_analysis.subtitle;
    analysis.content = high_level_analysis.content;
    analysis.maturing_state = MaturingState::Finished;
    let analysis = analysis.update(&pool)?;

    match last_analysis {
        Some(last_analysis) => {
            let mut last_analysis_resource = last_analysis.resource;
            last_analysis_resource.maturing_state = MaturingState::Trashed;
            let last_analysis_resource = last_analysis_resource.update(&pool)?;
            let mut new_parent_relation =
                NewResourceRelation::new(analysis.id, last_analysis_resource.id);
            new_parent_relation.user_id = Some(user_id);
            new_parent_relation.relation_type = Some("prnt".to_string());
            new_parent_relation.create(pool)?;
            Ok(analysis)
        }
        None => {
            Ok(analysis)
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