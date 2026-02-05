use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    resource::{
        entity_type::EntityType,
        maturing_state::MaturingState,
        resource_type::ResourceType,
        NewResource,
        Resource,
    },
    resource_relation::ResourceRelation,
};

use super::model::{NewTrace, Trace};

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
            entity_type: EntityType::Trace,
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
        Ok(Trace { journal_id, ..self })
    }

    /// Finds a Trace by id and fully hydrates it from the database.
    pub fn find_full_trace(id: Uuid, pool: &DbPool) -> Result<Trace, PpdcError> {
        let resource = Resource::find(id, pool)?;
        let trace = Trace::from_resource(resource);
        let trace = trace.with_user_id(pool)?;
        let trace = trace.with_journal(pool)?;
        Ok(trace)
    }
}

impl NewTrace {
    /// Converts to a NewResource for database insertion.
    pub fn to_new_resource(&self) -> NewResource {
        NewResource {
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: Some(self.content.clone()),
            resource_type: Some(ResourceType::Trace),
            entity_type: Some(EntityType::Trace),
            maturing_state: Some(MaturingState::Draft),
            publishing_state: Some("drft".to_string()),
            category_id: None,
            is_external: Some(false),
            external_content_url: None,
            comment: None,
            image_url: None,
        }
    }
}

impl From<Resource> for Trace {
    fn from(resource: Resource) -> Self {
        Trace::from_resource(resource)
    }
}
