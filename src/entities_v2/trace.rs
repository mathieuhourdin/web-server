use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::{Interaction, NewInteraction},
    resource::{maturing_state::MaturingState, resource_type::ResourceType, NewResource, Resource},
    resource_relation::{NewResourceRelation, ResourceRelation},
    session::Session,
};

use crate::work_analyzer::trace_broker::trace_qualify::qualify_trace;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct NewTraceDto {
    pub content: String,
    pub journal_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Trace {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub content: String,
    pub journal_id: Option<Uuid>,
    pub user_id: Uuid,
    pub maturing_state: MaturingState,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Trace {
    /// Creates a Trace from a Resource with default/placeholder values
    /// for fields that need to be hydrated from relations.
    pub fn from_resource(resource: Resource) -> Trace {
        Trace {
            id: resource.id,
            title: resource.title,
            subtitle: resource.subtitle,
            content: resource.content,
            interaction_date: None,
            user_id: Uuid::nil(),
            journal_id: None,
            maturing_state: resource.maturing_state,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }

    /// Converts the Trace back to a Resource.
    pub fn to_resource(&self) -> Resource {
        Resource {
            id: self.id,
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: self.content.clone(),
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_type: ResourceType::Trace,
            maturing_state: self.maturing_state,
            publishing_state: "drft".to_string(),
            category_id: None,
            is_external: false,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// Hydrates user_id and interaction_date from the author interaction.
    pub fn with_user_id(self, pool: &DbPool) -> Result<Trace, PpdcError> {
        let resource = self.to_resource();
        let interaction = resource.find_resource_author_interaction(pool)?;
        Ok(Trace {
            user_id: interaction.interaction_user_id,
            interaction_date: Some(interaction.interaction_date),
            ..self
        })
    }

    /// Hydrates journal_id from resource relations.
    /// Looks for a relation of type "jrit" (journal item) pointing to a journal.
    pub fn with_journal(self, pool: &DbPool) -> Result<Trace, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let journal_id = targets
            .into_iter()
            .find(|target| target.resource_relation.relation_type == "jrit")
            .map(|target| target.target_resource.id);
        Ok(Trace {
            journal_id,
            ..self
        })
    }

    /// Finds a Trace by id and fully hydrates it from the database.
    pub fn find_full_trace(id: Uuid, pool: &DbPool) -> Result<Trace, PpdcError> {
        println!("Finding full trace: {:?}", id);
        let resource = Resource::find(id, pool)?;
        let trace = Trace::from_resource(resource);
        let trace = trace.with_user_id(pool)?;
        let trace = trace.with_journal(pool)?;
        Ok(trace)
    }

    /// Updates the Trace in the database.
    pub fn update(self, pool: &DbPool) -> Result<Trace, PpdcError> {
        let resource = self.to_resource();
        let updated_resource = NewResource::from(resource).update(&self.id, pool)?;
        Ok(Trace {
            title: updated_resource.title,
            subtitle: updated_resource.subtitle,
            content: updated_resource.content,
            maturing_state: updated_resource.maturing_state,
            updated_at: updated_resource.updated_at,
            ..self
        })
    }

    /// Updates the maturing state of the trace.
    pub fn set_maturing_state(mut self, state: MaturingState, pool: &DbPool) -> Result<Trace, PpdcError> {
        self.maturing_state = state;
        self.update(pool)
    }

    pub fn get_between(user_id: Uuid, start_trace_id: Uuid, end_trace_id: Uuid, pool: &DbPool) -> Result<Vec<Trace>, PpdcError> {
        let start_trace = Trace::find_full_trace(start_trace_id, pool)?;
        let end_trace = Trace::find_full_trace(end_trace_id, pool)?;

        let interactions = Interaction::find_all_traces_for_user_between(
            user_id, 
            start_trace.interaction_date.unwrap(), 
            end_trace.interaction_date.unwrap(), 
            pool
        )?;

        let traces = interactions
            .into_iter()
            .map(|interaction| Trace::from_resource(interaction.resource))
            .collect();
        let mut traces: Vec<Trace> = traces;
        traces.push(end_trace);
        Ok(traces)
    }

    pub fn get_before(user_id: Uuid, trace_id: Uuid, pool: &DbPool) -> Result<Vec<Trace>, PpdcError> {
        let trace = Trace::find_full_trace(trace_id, pool)?;
        let interactions = Interaction::find_all_traces_for_user_before_date(user_id, trace.interaction_date.unwrap(), pool)?;
        let traces = interactions
            .into_iter()
            .map(|interaction| Trace::from_resource(interaction.resource))
            .collect();
        let mut traces: Vec<Trace> = traces;
        traces.push(trace);
        Ok(traces)
    }

    pub fn get_all_for_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<Trace>, PpdcError> {
        let interactions = Interaction::find_all_traces_for_user(user_id, pool)?;
        let traces = interactions
            .into_iter()
            .map(|interaction| Trace::from_resource(interaction.resource))
            .collect();
        Ok(traces)
    }
}

/// Struct for creating a new Trace with all required data.
pub struct NewTrace {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub user_id: Uuid,
    pub journal_id: Uuid,
}

impl NewTrace {
    /// Creates a new NewTrace with all fields.
    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        user_id: Uuid,
        journal_id: Uuid,
        interaction_date: Option<NaiveDateTime>,
    ) -> NewTrace {
        NewTrace {
            title,
            subtitle,
            content,
            interaction_date,
            user_id,
            journal_id,
        }
    }

    /// Converts to a NewResource for database insertion.
    fn to_new_resource(&self) -> NewResource {
        NewResource {
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: Some(self.content.clone()),
            resource_type: Some(ResourceType::Trace),
            maturing_state: Some(MaturingState::Draft),
            publishing_state: Some("drft".to_string()),
            category_id: None,
            is_external: Some(false),
            external_content_url: None,
            comment: None,
            image_url: None,
        }
    }

    /// Creates the Trace in the database.
    /// This creates the underlying Resource, Interaction, and ResourceRelation to journal.
    pub fn create(self, pool: &DbPool) -> Result<Trace, PpdcError> {
        let user_id = self.user_id;
        let interaction_date = self.interaction_date;
        let journal_id = self.journal_id;

        // Create the underlying resource
        let new_resource = self.to_new_resource();
        let created_resource = new_resource.create(pool)?;

        // Create the author interaction
        let mut new_interaction = NewInteraction::new(user_id, created_resource.id);
        new_interaction.interaction_type = Some("outp".to_string());
        if let Some(date) = interaction_date {
            new_interaction.interaction_date = Some(date);
        }
        new_interaction.interaction_progress = 0;
        new_interaction.create(pool)?;

        // Create journal relation
        let mut journal_relation = NewResourceRelation::new(created_resource.id, journal_id);
        journal_relation.relation_type = Some("jrit".to_string());
        journal_relation.user_id = Some(user_id);
        journal_relation.create(pool)?;

        // Return the fully hydrated trace
        Trace::find_full_trace(created_resource.id, pool)
    }
}

#[debug_handler]
pub async fn post_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewTraceDto>,
) -> Result<Json<Resource>, PpdcError> {
    let journal_interaction =
        Interaction::find_user_journal(session.user_id.unwrap(), payload.journal_id, &pool)?;

    let (new_resource, interaction_date) = qualify_trace(payload.content.as_str()).await?;

    let resource = new_resource.create(&pool)?;
    let mut new_interaction = NewInteraction::new(session.user_id.unwrap(), resource.id);

    if let Some(interaction_date) = interaction_date {
        new_interaction.interaction_date = Some(interaction_date.and_hms_opt(12, 0, 0).unwrap());
    }
    new_interaction.create(&pool)?;

    let mut new_resource_relation =
        NewResourceRelation::new(resource.id, journal_interaction.resource_id.unwrap());
    new_resource_relation.user_id = session.user_id;
    new_resource_relation.relation_type = Some("jrit".to_string());
    new_resource_relation.create(&pool)?;

    //let element_resources = create_element_resources_from_trace(resource.clone(), session.user_id.unwrap(), &pool).await?;

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