use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::{Interaction, NewInteraction},
    resource::Resource,
    resource_relation::NewResourceRelation,
    session::Session,
};
use crate::entities_v2::lens::Lens;
use crate::work_analyzer;

use super::{
    llm_qualify,
    model::{NewTraceDto, Trace},
};

#[debug_handler]
pub async fn post_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewTraceDto>,
) -> Result<Json<Resource>, PpdcError> {
    let user_id = session.user_id.unwrap();
    let journal_interaction = Interaction::find_user_journal(user_id, payload.journal_id, &pool)?;

    let (new_resource, interaction_date) =
        llm_qualify::qualify_trace(payload.content.as_str()).await?;

    let resource = new_resource.create(&pool)?;
    let mut new_interaction = NewInteraction::new(user_id, resource.id);

    if let Some(interaction_date) = interaction_date {
        new_interaction.interaction_date = Some(interaction_date.and_hms_opt(12, 0, 0).unwrap());
    }
    new_interaction.create(&pool)?;

    let mut new_resource_relation =
        NewResourceRelation::new(resource.id, journal_interaction.resource_id.unwrap());
    new_resource_relation.user_id = Some(user_id);
    new_resource_relation.relation_type = Some("jrit".to_string());
    new_resource_relation.create(&pool)?;

    let user_lenses = Lens::get_user_lenses(user_id, &pool)?;
    for lens in user_lenses.into_iter().filter(|lens| lens.autoplay) {
        let lens = if lens.target_trace_id != resource.id {
            lens.update_target_trace(resource.id, &pool)?
        } else {
            lens
        };
        tokio::spawn(async move { work_analyzer::run_lens(lens.id).await });
    }

    Ok(Json(resource))
}

#[debug_handler]
pub async fn get_all_traces_for_user_route(
    Extension(pool): Extension<DbPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<Trace>>, PpdcError> {
    let traces = Trace::get_all_for_user(user_id, &pool)?;
    Ok(Json(traces))
}

#[debug_handler]
pub async fn get_trace_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Trace>, PpdcError> {
    let trace = Trace::find_full_trace(id, &pool)?;
    Ok(Json(trace))
}

#[debug_handler]
pub async fn get_traces_for_journal_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Trace>>, PpdcError> {
    let traces = Trace::get_all_for_journal(id, &pool)?;
    Ok(Json(traces))
}
