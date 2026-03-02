use std::collections::HashSet;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::Interaction,
    resource::{EntityType, NewResource, Resource, ResourceType},
    resource_relation::ResourceRelation,
};
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;

use super::model::{Lens, LensProcessingState, NewLens};

impl Lens {
    pub fn from_resource(resource: Resource) -> Lens {
        Lens {
            id: resource.id,
            user_id: None,
            processing_state: LensProcessingState::from_maturing_state(resource.maturing_state),
            fork_landscape_id: None,
            current_landscape_id: None,
            target_trace_id: None,
            autoplay: resource.is_external,
        }
    }

    pub fn with_user_id(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let resource = Resource::find(self.id, pool)?;
        let interaction = resource.find_resource_author_interaction(&pool)?;
        let user_id = interaction.interaction_user_id;
        Ok(Lens { user_id: Some(user_id), ..self })
    }
    pub fn with_forked_landscape(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, &pool)?;
        let fork_landscape_id = targets
            .into_iter()
            .find(|target| target.resource_relation.relation_type == "fork".to_string())
            .map(|target| target.target_resource.id);
        Ok(Lens {
            fork_landscape_id,
            ..self
        })
    }
    pub fn with_current_landscape(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, &pool)?;
        let current_landscape_id = targets
            .into_iter()
            .find(|target| target.resource_relation.relation_type == "head".to_string())
            .map(|target| target.target_resource.id);
        Ok(Lens {
            current_landscape_id,
            ..self
        })
    }
    pub fn with_target_trace(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, &pool)?;
        let target_trace_id = targets
            .into_iter()
            .find(|target| target.resource_relation.relation_type == "trgt".to_string())
            .map(|target| target.target_resource.id);
        Ok(Lens {
            target_trace_id,
            ..self
        })
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
        let lenses = lenses
            .into_iter()
            .map(|interaction| Lens::find_full_lens(interaction.resource.id, pool).unwrap())
            .collect::<Vec<Lens>>();
        Ok(lenses)
    }

    pub fn get_analysis_scope_ids(&self, pool: &DbPool) -> Result<Vec<Uuid>, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let mut seen = HashSet::new();
        let analysis_ids = targets
            .into_iter()
            .filter(|target| {
                target.resource_relation.relation_type == "lnsa"
                    && target.target_resource.is_landscape_analysis()
            })
            .filter_map(|target| {
                let target_id = target.target_resource.id;
                if seen.insert(target_id) {
                    Some(target_id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        Ok(analysis_ids)
    }

    pub fn get_analysis_scope(&self, pool: &DbPool) -> Result<Vec<LandscapeAnalysis>, PpdcError> {
        let analysis_ids = self.get_analysis_scope_ids(pool)?;
        analysis_ids
            .into_iter()
            .map(|analysis_id| LandscapeAnalysis::find_full_analysis(analysis_id, pool))
            .collect::<Result<Vec<_>, _>>()
    }
}

impl NewLens {
    pub fn to_new_resource(self) -> NewResource {
        NewResource {
            title: "Lens".to_string(),
            subtitle: "".to_string(),
            content: None,
            resource_type: Some(ResourceType::Lens),
            entity_type: Some(EntityType::Lens),
            maturing_state: Some(self.processing_state.to_maturing_state()),
            publishing_state: Some("drft".to_string()),
            is_external: Some(self.autoplay),
            external_content_url: None,
            comment: None,
            image_url: None,
            resource_subtype: None,
        }
    }
}

impl From<Resource> for Lens {
    fn from(resource: Resource) -> Self {
        Lens::from_resource(resource)
    }
}
