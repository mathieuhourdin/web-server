use uuid::Uuid;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    resource::{Resource, NewResource, resource_type::ResourceType, maturing_state::MaturingState},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Journal {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub user_id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Journal {
    /// Creates a Journal from a Resource with default/placeholder values
    /// for fields that need to be hydrated from relations.
    pub fn from_resource(resource: Resource) -> Journal {
        Journal {
            id: resource.id,
            title: resource.title,
            subtitle: resource.subtitle,
            content: resource.content,
            user_id: Uuid::nil(),
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }

    /// Converts the Journal back to a Resource.
    pub fn to_resource(&self) -> Resource {
        Resource {
            id: self.id,
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: self.content.clone(),
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_type: ResourceType::Journal,
            maturing_state: MaturingState::Draft,
            publishing_state: "drft".to_string(),
            category_id: None,
            is_external: false,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// Finds a Journal by id (basic retrieval without hydration).
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Journal, PpdcError> {
        let resource = Resource::find(id, pool)?;
        Ok(Journal::from_resource(resource))
    }

    /// Hydrates user_id from the author interaction.
    pub fn with_user_id(self, pool: &DbPool) -> Result<Journal, PpdcError> {
        let resource = self.to_resource();
        let interaction = resource.find_resource_author_interaction(pool)?;
        Ok(Journal {
            user_id: interaction.interaction_user_id,
            ..self
        })
    }

    /// Finds a Journal by id and fully hydrates it from the database.
    /// This retrieves all related fields from interactions and resource_relations.
    pub fn find_full(id: Uuid, pool: &DbPool) -> Result<Journal, PpdcError> {
        let resource = Resource::find(id, pool)?;
        let journal = Journal::from_resource(resource);
        let journal = journal.with_user_id(pool)?;
        Ok(journal)
    }

    /// Updates the Journal in the database.
    pub fn update(self, pool: &DbPool) -> Result<Journal, PpdcError> {
        let resource = self.to_resource();
        let updated_resource = NewResource::from(resource).update(&self.id, pool)?;
        Ok(Journal {
            title: updated_resource.title,
            subtitle: updated_resource.subtitle,
            content: updated_resource.content,
            updated_at: updated_resource.updated_at,
            ..self
        })
    }
}