use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError, interaction::model::Interaction, resource::maturing_state::MaturingState,
    resource::resource_type::ResourceType, resource::Resource,
};
use crate::schema::{interactions, resource_relations, resources};

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
    pub trace_type: TraceType,
    pub maturing_state: MaturingState,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceType {
    BioTrace,
    WorkspaceTrace,
    UserTrace,
    #[serde(rename = "HIGH_LEVEL_PROJECTS_DEFINITION")]
    HighLevelProjectsDefinition,
}

impl From<ResourceType> for TraceType {
    fn from(resource_type: ResourceType) -> Self {
        match resource_type {
            ResourceType::BioTrace => TraceType::BioTrace,
            ResourceType::WorkspaceTrace => TraceType::WorkspaceTrace,
            ResourceType::UserTrace => TraceType::UserTrace,
            ResourceType::HighLevelProjectsDefinitionTrace => {
                TraceType::HighLevelProjectsDefinition
            }
            // Backward compatibility for existing trace rows.
            ResourceType::Trace => TraceType::UserTrace,
            _ => TraceType::UserTrace,
        }
    }
}

impl From<TraceType> for ResourceType {
    fn from(trace_type: TraceType) -> Self {
        match trace_type {
            TraceType::BioTrace => ResourceType::BioTrace,
            TraceType::WorkspaceTrace => ResourceType::WorkspaceTrace,
            TraceType::UserTrace => ResourceType::UserTrace,
            TraceType::HighLevelProjectsDefinition => {
                ResourceType::HighLevelProjectsDefinitionTrace
            }
        }
    }
}

impl Trace {
    fn find_first_user_trace_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Trace>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let first_user_trace = interactions::table
            .inner_join(resources::table)
            .filter(interactions::interaction_user_id.eq(user_id))
            .filter(interactions::interaction_type.eq("outp"))
            .filter(resources::resource_type.eq_any(vec![
                ResourceType::UserTrace,
                // Backward compatibility for legacy trace rows.
                ResourceType::Trace,
            ]))
            .order(interactions::interaction_date.asc())
            .select((Interaction::as_select(), Resource::as_select()))
            .first::<(Interaction, Resource)>(&mut conn)
            .optional()?;

        match first_user_trace {
            Some((_, resource)) => Ok(Some(Trace::find_full_trace(resource.id, pool)?)),
            None => Ok(None),
        }
    }

    pub fn get_most_recent_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Trace>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let latest_trace = interactions::table
            .inner_join(resources::table)
            .filter(interactions::interaction_user_id.eq(user_id))
            .filter(interactions::interaction_type.eq("outp"))
            .filter(resources::resource_type.eq_any(vec![
                ResourceType::UserTrace,
                ResourceType::BioTrace,
                ResourceType::WorkspaceTrace,
                ResourceType::HighLevelProjectsDefinitionTrace,
            ]))
            .order(interactions::interaction_date.desc())
            .select((Interaction::as_select(), Resource::as_select()))
            .first::<(Interaction, Resource)>(&mut conn)
            .optional()?;

        match latest_trace {
            Some((_, resource)) => Ok(Some(Trace::find_full_trace(resource.id, pool)?)),
            None => Ok(None),
        }
    }

    pub fn get_between(
        user_id: Uuid,
        start_trace_id: Uuid,
        end_trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Trace>, PpdcError> {
        let start_trace = Trace::find_full_trace(start_trace_id, pool)?;
        let end_trace = Trace::find_full_trace(end_trace_id, pool)?;

        let interactions = Interaction::find_all_traces_for_user_between(
            user_id,
            start_trace.interaction_date.unwrap(),
            end_trace.interaction_date.unwrap(),
            pool,
        )?;

        let traces = interactions
            .into_iter()
            .map(|interaction| Trace::from_resource(interaction.resource))
            .collect();
        let mut traces: Vec<Trace> = traces;
        traces.push(end_trace);
        Ok(traces)
    }

    pub fn get_before(
        user_id: Uuid,
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Trace>, PpdcError> {
        let trace = Trace::find_full_trace(trace_id, pool)?;
        let interactions = Interaction::find_all_traces_for_user_before_date(
            user_id,
            trace.interaction_date.unwrap(),
            pool,
        )?;
        let traces = interactions
            .into_iter()
            .map(|interaction| Trace::from_resource(interaction.resource))
            .collect();
        let mut traces: Vec<Trace> = traces;
        traces.push(trace);
        Ok(traces)
    }

    pub fn get_first(user_id: Uuid, pool: &DbPool) -> Result<Option<Trace>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let latest_hlp_trace = interactions::table
            .inner_join(resources::table)
            .filter(interactions::interaction_user_id.eq(user_id))
            .filter(interactions::interaction_type.eq("outp"))
            .filter(
                resources::resource_type.eq(ResourceType::HighLevelProjectsDefinitionTrace),
            )
            .order(interactions::interaction_date.desc())
            .select((Interaction::as_select(), Resource::as_select()))
            .first::<(Interaction, Resource)>(&mut conn)
            .optional()?;

        if let Some((_, resource)) = latest_hlp_trace {
            return Ok(Some(Trace::find_full_trace(resource.id, pool)?));
        }

        let latest_bio_trace = interactions::table
            .inner_join(resources::table)
            .filter(interactions::interaction_user_id.eq(user_id))
            .filter(interactions::interaction_type.eq("outp"))
            .filter(resources::resource_type.eq(ResourceType::BioTrace))
            .order(interactions::interaction_date.desc())
            .select((Interaction::as_select(), Resource::as_select()))
            .first::<(Interaction, Resource)>(&mut conn)
            .optional()?;

        if let Some((_, resource)) = latest_bio_trace {
            return Ok(Some(Trace::find_full_trace(resource.id, pool)?));
        }

        Trace::find_first_user_trace_for_user(user_id, pool)
    }

    pub fn get_next(
        user_id: Uuid,
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Trace>, PpdcError> {
        let trace = Trace::find_full_trace(trace_id, pool)?;
        if trace.trace_type == TraceType::HighLevelProjectsDefinition {
            let mut conn = pool
                .get()
                .expect("Failed to get a connection from the pool");
            let latest_bio_trace = interactions::table
                .inner_join(resources::table)
                .filter(interactions::interaction_user_id.eq(user_id))
                .filter(interactions::interaction_type.eq("outp"))
                .filter(resources::resource_type.eq(ResourceType::BioTrace))
                .order(interactions::interaction_date.desc())
                .select((Interaction::as_select(), Resource::as_select()))
                .first::<(Interaction, Resource)>(&mut conn)
                .optional()?;

            if let Some((_, resource)) = latest_bio_trace {
                return Ok(Some(Trace::find_full_trace(resource.id, pool)?));
            }

            return Trace::find_first_user_trace_for_user(user_id, pool);
        }

        if trace.trace_type == TraceType::BioTrace {
            return Trace::find_first_user_trace_for_user(user_id, pool);
        }

        let interactions = Interaction::find_all_traces_for_user_after_date(
            user_id,
            trace.interaction_date.unwrap(),
            pool,
        )?;
        let next_trace = interactions
            .into_iter()
            .map(|interaction| Trace::from_resource(interaction.resource))
            .filter(|candidate| candidate.trace_type == TraceType::UserTrace)
            .next();
        match next_trace {
            Some(next_trace) => Ok(Some(Trace::find_full_trace(next_trace.id, pool)?)),
            None => Ok(None),
        }
    }

    pub fn get_all_for_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<Trace>, PpdcError> {
        let interactions = Interaction::find_all_traces_for_user(user_id, pool)?;
        let traces = interactions
            .into_iter()
            .map(|interaction| Trace::from_resource(interaction.resource))
            .collect();
        Ok(traces)
    }

    pub fn get_all_for_journal(journal_id: Uuid, pool: &DbPool) -> Result<Vec<Trace>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let interactions_with_resources = interactions::table
            .inner_join(resources::table)
            .inner_join(
                resource_relations::table
                    .on(resources::id.eq(resource_relations::origin_resource_id)),
            )
            .filter(resource_relations::target_resource_id.eq(journal_id))
            .filter(resource_relations::relation_type.eq("jrit"))
            .filter(resources::resource_type.eq_any(vec![
                ResourceType::UserTrace,
                ResourceType::BioTrace,
                ResourceType::WorkspaceTrace,
                ResourceType::HighLevelProjectsDefinitionTrace,
            ]))
            .filter(interactions::interaction_type.eq("outp"))
            .order(interactions::interaction_date.desc())
            .select((Interaction::as_select(), Resource::as_select()))
            .load::<(Interaction, Resource)>(&mut conn)?;

        let traces = interactions_with_resources
            .into_iter()
            .map(|(interaction, resource)| {
                let trace = Trace::from_resource(resource);
                Trace {
                    interaction_date: Some(interaction.interaction_date),
                    user_id: interaction.interaction_user_id,
                    journal_id: Some(journal_id),
                    ..trace
                }
            })
            .collect::<Vec<Trace>>();
        Ok(traces)
    }
}

/// Struct for creating a new Trace with all required data.
#[derive(Debug, Clone)]
pub struct NewTrace {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub user_id: Uuid,
    pub trace_type: TraceType,
    pub journal_id: Uuid,
}

impl NewTrace {
    /// Creates a new NewTrace with all fields.
    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        interaction_date: Option<NaiveDateTime>,
        user_id: Uuid,
        journal_id: Uuid,
    ) -> NewTrace {
        NewTrace {
            title,
            subtitle,
            content,
            interaction_date,
            user_id,
            trace_type: TraceType::UserTrace,
            journal_id,
        }
    }
}
