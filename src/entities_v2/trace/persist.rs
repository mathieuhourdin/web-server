use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::NewInteraction,
    resource::NewResource,
    resource_relation::NewResourceRelation,
};

use super::model::{NewTrace, Trace};

impl Trace {
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
    pub fn set_maturing_state(mut self, state: crate::entities::resource::maturing_state::MaturingState, pool: &DbPool) -> Result<Trace, PpdcError> {
        self.maturing_state = state;
        self.update(pool)
    }
}

impl NewTrace {
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
