use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::Interaction,
    resource::maturing_state::MaturingState,
};

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
#[derive(Debug, Clone)]
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
            journal_id,
        }
    }
}
