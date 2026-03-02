use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::NewInteraction,
    resource::{entity_type::EntityType, NewResource},
};

use super::model::{Journal, JournalStatus, NewJournalDto};

impl Journal {
    /// Updates the Journal in the database.
    pub fn update(self, pool: &DbPool) -> Result<Journal, PpdcError> {
        let resource = self.to_resource();
        let updated_resource = NewResource::from(resource).update(&self.id, pool)?;
        Ok(Journal {
            title: updated_resource.title,
            subtitle: updated_resource.subtitle,
            content: updated_resource.content,
            journal_type: updated_resource.resource_type.into(),
            status: if updated_resource.maturing_state
                == crate::entities::resource::maturing_state::MaturingState::Trashed
            {
                JournalStatus::Archived
            } else if updated_resource.publishing_state == "pbsh"
                || updated_resource.maturing_state
                    == crate::entities::resource::maturing_state::MaturingState::Finished
            {
                JournalStatus::Published
            } else {
                JournalStatus::Draft
            },
            updated_at: updated_resource.updated_at,
            ..self
        })
    }

    pub fn create(payload: NewJournalDto, user_id: uuid::Uuid, pool: &DbPool) -> Result<Journal, PpdcError> {
        let journal_type = payload.journal_type.unwrap_or(super::model::JournalType::WorkLogJournal);
        let resource = NewResource {
            title: payload.title,
            subtitle: payload.subtitle.unwrap_or_default(),
            content: Some(payload.content.unwrap_or_default()),
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_type: Some(journal_type.into()),
            maturing_state: Some(crate::entities::resource::maturing_state::MaturingState::Draft),
            publishing_state: Some("drft".to_string()),
            is_external: Some(false),
            entity_type: Some(EntityType::Journal),
            resource_subtype: None,
        }
        .create(pool)?;

        let mut new_interaction = NewInteraction::new(user_id, resource.id);
        new_interaction.interaction_type = Some("outp".to_string());
        new_interaction.create(pool)?;

        Journal::find_full(resource.id, pool)
    }
}
