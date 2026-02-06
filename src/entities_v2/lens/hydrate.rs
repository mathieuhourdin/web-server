use chrono::NaiveDateTime;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    interaction::model::Interaction,
    resource::{
        EntityType,
        NewResource,
        Resource,
        ResourceType,
    },
    resource_relation::ResourceRelation,
};

use super::model::{Lens, NewLens};

impl Lens {
    pub fn from_resource(resource: Resource) -> Lens {
        Lens {
            id: resource.id,
            user_id: None,
            name: resource.title,
            description: resource.subtitle,
            processing_state: resource.maturing_state,
            fork_landscape_id: None,
            current_landscape_id: None,
            target_trace_id: Uuid::nil(),
            #[allow(deprecated)]
            current_state_date: NaiveDateTime::from_timestamp_opt(0, 0).unwrap(),
            model_version: "".to_string(),
            autoplay: false,
            is_primary: false,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }
    pub fn to_resource(self) -> Resource {
        Resource {
            id: self.id,
            title: self.name,
            subtitle: self.description,
            content: "".to_string(),
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_type: ResourceType::Lens,
            entity_type: EntityType::Lens,
            maturing_state: self.processing_state,
            publishing_state: "drft".to_string(),
            category_id: None,
            is_external: false,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
    pub fn with_user_id(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let resource = self.clone().to_resource();
        let interaction = resource.find_resource_author_interaction(&pool)?;
        let user_id = interaction.interaction_user_id;
        let date = interaction.interaction_date;
        Ok(Lens {
            user_id: Some(user_id),
            current_state_date: date,
            ..self
        })
    }
    pub fn with_forked_landscape(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, &pool)?;
        let fork_landscape_id = targets.into_iter()
            .find(|target| target.resource_relation.relation_type == "fork".to_string())
            .map(|target| target.target_resource.id);
        Ok(Lens { fork_landscape_id, ..self })
    }
    pub fn with_current_landscape(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, &pool)?;
        let current_landscape_id = targets.into_iter()
            .find(|target| target.resource_relation.relation_type == "head".to_string())
            .map(|target| target.target_resource.id);
        Ok(Lens { current_landscape_id, ..self })
    }
    pub fn with_target_trace(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, &pool)?;
        let target_trace_id = targets.into_iter()
            .find(|target| target.resource_relation.relation_type == "trgt".to_string())
            .map(|target| target.target_resource.id);
        if target_trace_id.is_none() {
            return Err(PpdcError::new(404, ErrorType::ApiError, "Target trace not found".to_string()));
        }
        let target_trace_id = target_trace_id.unwrap();
        Ok(Lens { target_trace_id, ..self })
    }
    pub fn find_full_lens(id: Uuid, pool: &DbPool) -> Result<Lens, PpdcError> {
        let lens = Resource::find(id, pool)?;
        let lens = Lens::from_resource(lens);
        let lens = lens.with_user_id(pool)?;
        let lens = lens.with_forked_landscape(pool)?;
        let lens = lens.with_current_landscape(pool)?;
        let lens = lens.with_target_trace(pool)?;
        Ok(lens)
    }

    pub fn get_user_lenses(user_id: Uuid, pool: &DbPool) -> Result<Vec<Lens>, PpdcError> {
        let lenses = Interaction::find_paginated_outputs_for_user(0, 200, user_id, "lens", pool)?;
        let lenses = lenses.into_iter()
            .map(|interaction| Lens::find_full_lens(interaction.resource.id, pool).unwrap())
            .collect::<Vec<Lens>>();
        Ok(lenses)
    }
}

impl NewLens {
    pub fn to_new_resource(self) -> NewResource {
        NewResource {
            title: self.name,
            subtitle: self.description,
            content: None,
            resource_type: Some(ResourceType::Lens),
            entity_type: Some(EntityType::Lens),
            maturing_state: Some(self.processing_state),
            publishing_state: Some("drft".to_string()),
            category_id: None,
            is_external: None,
            external_content_url: None,
            comment: None,
            image_url: None,
        }
    }
}

impl From<Resource> for Lens {
    fn from(resource: Resource) -> Self {
        Lens::from_resource(resource)
    }
}
