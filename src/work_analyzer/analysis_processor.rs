use crate::entities::{
    analysis::find_last_analysis_resource, 
    resource_relation::NewResourceRelation,
    resource::{
        Resource, 
        NewResource, 
        resource_type::ResourceType, 
        maturing_state::MaturingState}, 
    error::{PpdcError, ErrorType}, 
    interaction::model::{
        Interaction, 
        InteractionWithResource, 
        NewInteraction
    }};
use crate::db::get_global_pool;
use crate::openai_handler::gpt_handler::make_gpt_request;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::NaiveDate;
use crate::db::DbPool;
use chrono::Duration;
use crate::work_analyzer::{
    context_builder::{build_context, get_landmarks_from_analysis},
    trace_broker::{create_element_resources_from_trace, SimpleElement},
    update_landmarks::{create_landmarks_and_link_elements, create_landmarks_from_string_traces, AnalysisEntity},
    match_elements_and_landmarks::{match_elements_and_landmarks, LandmarkPojectionWithElements}
};



pub async fn analyse_to_date(user_id: Uuid, date: NaiveDate, pool: &DbPool) -> Result<Resource, PpdcError> {

    // we find the last analysis for the user
    let last_analysis = find_last_analysis_resource(user_id, &pool)?;

    let mut last_analysis_id_option = None;

    let analysis_title = match last_analysis {
        Some(last_analysis) => {
            last_analysis_id_option = Some(last_analysis.resource.id);
            if last_analysis.interaction.interaction_date.unwrap() > (date - Duration::days(1)).and_hms_opt(12, 0, 0).unwrap() {
                return Err(PpdcError::new(400, ErrorType::ApiError, "Analysis already exists for this day".to_string()));
            } else {
                format!("Analyse du {} au {}", last_analysis.interaction.interaction_date.unwrap().format("%Y-%m-%d").to_string(), date.and_hms_opt(12, 0, 0).unwrap().format("%Y-%m-%d").to_string())
            }
        }
        None => {
            format!("Première Analyse, jusqu'au {}", date.and_hms_opt(12, 0, 0).unwrap().format("%Y-%m-%d").to_string())
        }
    };
    // we create a new analysis resource that will be the support of all performed analysis in this analysis session
    let analysis_resource = NewResource::new(
        analysis_title, 
        "Analyse".to_string(), 
        "Analyse".to_string(), 
        ResourceType::Analysis);
    let mut analysis_resource = analysis_resource.create(&pool)?;

    // we create a new interaction for the analysis resource, with the given date
    let mut new_interaction = NewInteraction::new(user_id, analysis_resource.id);
    new_interaction.interaction_type = Some("anly".to_string());
    new_interaction.interaction_date = Some(date.and_hms_opt(12, 0, 0).unwrap());
    new_interaction.create(&pool)?;

    let resources = process_analysis(user_id, date, &analysis_resource.id, last_analysis_id_option, &pool).await?;

    // if everything is ok, we update the resource to finished state
    analysis_resource.maturing_state = MaturingState::Finished;
    let result = analysis_resource.update(&pool)?;

    Ok(result)
}

pub async fn process_analysis(user_id: Uuid, date: NaiveDate, analysis_resource_id: &Uuid, last_analysis_id_option: Option<Uuid>, pool: &DbPool) -> Result<(Vec<Resource>, Vec<Resource>), PpdcError> {
    let interactions = Interaction::find_all_draft_traces_for_user_before_date(user_id, date, pool)?;

    // if no new traces found, we dont process the analysis
    if interactions.is_empty() {
        return Ok((vec![], vec![]));
    }
    println!("Interactions: {:?}", interactions);

    let (analysis_result) = process_traces(&interactions, analysis_resource_id, last_analysis_id_option, user_id, pool).await?;

    for interaction in interactions {
        let mut new_resource_relation = NewResourceRelation::new(interaction.resource.id, analysis_resource_id.clone());
        new_resource_relation.user_id = Some(user_id);
        new_resource_relation.relation_type = Some("srcs".to_string());
        new_resource_relation.create(pool)?;
        let mut trace = interaction.resource;
        trace.maturing_state = MaturingState::Finished;
        trace.update(pool)?;
    }
    
    Ok(analysis_result)
}

pub async fn process_traces(traces: &Vec<InteractionWithResource>, analysis_resource_id: &Uuid, last_analysis_id_option: Option<Uuid>, user_id: Uuid, pool: &DbPool) -> Result<(Vec<Resource>, Vec<Resource>), PpdcError> {
    // here we process the traces using the trace broker to identify simple elements that will be used to create analysis landmarks
    let string_traces = traces.iter().map(|interaction| format!("Date : {}\nTitre : {}\nSous titre : {}\nContenu : {}", interaction.interaction.interaction_date.unwrap().format("%Y-%m-%d").to_string(), interaction.resource.title, interaction.resource.subtitle, interaction.resource.content)).collect::<Vec<_>>().join("\n\n");
    let mut simple_elements: Vec<Resource> = vec![];

    // we create the simple elements from the traces and from the context.
    let context = build_context(&user_id, last_analysis_id_option, &pool)?;

    for trace in traces {
        let processed_elements = create_element_resources_from_trace(trace.resource.clone(), user_id, context.clone(), analysis_resource_id.clone(), pool).await?;
        simple_elements.extend(processed_elements);
    }

    // we match the simple elements to the analysis landmarks.
    let analysis_landmarks = get_landmarks_from_analysis(last_analysis_id_option, &pool)?;
    let landmarks = match_elements_and_landmarks(&simple_elements, &analysis_landmarks, &context).await?;

    // we update the landmarks and create the new landmarks, and create the relation with the elements.
    let created_landmarks = create_landmarks_and_link_elements(&landmarks, &simple_elements, analysis_resource_id, &user_id, pool)?;
    //let analysis_entities: Vec<AnalysisEntity> = create_landmarks_from_string_traces(string_traces).await?;
    //let new_landmarks = create_resources_from_analysis(analysis_entities, user_id, analysis_resource_id).await?;
    Ok((simple_elements, created_landmarks))
}


pub async fn match_simple_elements_to_analytic_landmarks(simple_elements: Vec<Resource>, analysis_resource_id: &Uuid) -> Result<Vec<Resource>, PpdcError> {
    let landmarks = vec![];
    Ok(landmarks)
}

pub async fn create_new_analytic_landmarks(string_traces: String, existing_landmarks: Vec<Resource>, user_id: Uuid, analysis_resource_id: &Uuid) -> Result<Vec<Resource>, PpdcError> {
    Ok(existing_landmarks)
}


pub async fn create_resources_from_analysis(analysis_entities: Vec<AnalysisEntity>, user_id: Uuid, analysis_resource_id: &Uuid) -> Result<Vec<Resource>, PpdcError> {
    let pool = get_global_pool();
    let resources = analysis_entities.iter().map(|entity| {
        let new_resource = NewResource::new(entity.title.clone(), entity.subtitle.clone(), entity.content.clone(), ResourceType::from_code(entity.entity_type.as_str())?);
        let resource = new_resource.create(pool)?;
        let mut new_interaction = NewInteraction::new(user_id, resource.id);
        new_interaction.interaction_type = Some("anly".to_string());
        new_interaction.interaction_progress = entity.progress;
        new_interaction.create(pool)?;
        let mut new_resource_relation = NewResourceRelation::new(resource.id, analysis_resource_id.clone());
        new_resource_relation.user_id = Some(user_id);
        new_resource_relation.relation_type = Some("ownr".to_string());
        new_resource_relation.create(pool)?;
        Ok(resource)
    }).collect::<Result<Vec<Resource>, PpdcError>>()?;
    Ok(resources)
}