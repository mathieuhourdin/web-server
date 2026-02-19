use std::collections::HashSet;

use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::Interaction,
    resource::{
        entity_type::EntityType, maturing_state::MaturingState, resource_type::ResourceType,
        NewResource, Resource,
    },
    resource_relation::ResourceRelation,
};
use crate::entities_v2::landmark::{Landmark, LandmarkType};
use crate::entities_v2::reference::Reference;

use super::model::{NewTraceMirror, TraceMirror};

impl TraceMirror {
    /// Creates a TraceMirror from a Resource with default/placeholder values
    /// for fields that need to be hydrated from relations.
    pub fn from_resource(resource: Resource) -> TraceMirror {
        // Parse tags from comment (stored as JSON array)
        let tags = if let Some(comment) = &resource.comment {
            if !comment.is_empty() {
                serde_json::from_str(comment).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        TraceMirror {
            id: resource.id,
            title: resource.title,
            subtitle: resource.subtitle,
            content: resource.content,
            tags,
            trace_id: Uuid::nil(),
            landscape_analysis_id: Uuid::nil(),
            user_id: Uuid::nil(),
            primary_resource_id: None,
            primary_theme_id: None,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }

    /// Converts the TraceMirror back to a Resource.
    pub fn to_resource(&self) -> Resource {
        // Serialize tags to JSON for storage in comment field
        let tags_json = serde_json::to_string(&self.tags).unwrap_or_else(|_| "[]".to_string());

        Resource {
            id: self.id,
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: self.content.clone(),
            external_content_url: None,
            comment: Some(tags_json),
            image_url: None,
            resource_type: ResourceType::TraceMirror,
            entity_type: EntityType::TraceMirror,
            maturing_state: MaturingState::Draft,
            publishing_state: "drft".to_string(),
            category_id: None,
            is_external: false,
            created_at: self.created_at,
            updated_at: self.updated_at,
            resource_subtype: None,
        }
    }

    /// Hydrates user_id from the author interaction.
    pub fn with_user_id(self, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let resource = self.to_resource();
        let interaction = resource.find_resource_author_interaction(pool)?;
        Ok(TraceMirror {
            user_id: interaction.interaction_user_id,
            ..self
        })
    }

    /// Hydrates trace_id from resource relations.
    /// Looks for a relation of type "trce" (trace) pointing to a trace.
    pub fn with_trace(self, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let trace_id = targets
            .into_iter()
            .find(|target| target.resource_relation.relation_type == "trce")
            .map(|target| target.target_resource.id)
            .unwrap_or(Uuid::nil());
        Ok(TraceMirror { trace_id, ..self })
    }

    /// Hydrates landscape_analysis_id from resource relations.
    /// Looks for a relation of type "lnds" (landscape) pointing to a landscape analysis.
    pub fn with_landscape_analysis(self, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let landscape_analysis_id = targets
            .into_iter()
            .find(|target| target.resource_relation.relation_type == "lnds")
            .map(|target| target.target_resource.id)
            .unwrap_or(Uuid::nil());
        Ok(TraceMirror {
            landscape_analysis_id,
            ..self
        })
    }

    /// Hydrates primary_resource_id from resource relations.
    /// Looks for a relation of type "prir" (primary resource).
    pub fn with_primary_resource(self, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let primary_resource_id = targets
            .into_iter()
            .find(|target| target.resource_relation.relation_type == "prir")
            .map(|target| target.target_resource.id);
        Ok(TraceMirror {
            primary_resource_id,
            ..self
        })
    }

    /// Hydrates primary_theme_id from resource relations.
    /// Looks for a relation of type "prit" (primary theme).
    pub fn with_primary_theme(self, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let primary_theme_id = targets
            .into_iter()
            .find(|target| target.resource_relation.relation_type == "prit")
            .map(|target| target.target_resource.id);
        Ok(TraceMirror {
            primary_theme_id,
            ..self
        })
    }

    /// Finds a TraceMirror by id and fully hydrates it from the database.
    pub fn find_full_trace_mirror(id: Uuid, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let resource = Resource::find(id, pool)?;
        let trace_mirror = TraceMirror::from_resource(resource);
        let trace_mirror = trace_mirror.with_user_id(pool)?;
        let trace_mirror = trace_mirror.with_trace(pool)?;
        let trace_mirror = trace_mirror.with_landscape_analysis(pool)?;
        let trace_mirror = trace_mirror.with_primary_resource(pool)?;
        let trace_mirror = trace_mirror.with_primary_theme(pool)?;
        Ok(trace_mirror)
    }

    /// Get all trace mirrors for a specific landscape analysis
    pub fn find_by_landscape_analysis(
        landscape_analysis_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<TraceMirror>, PpdcError> {
        let origins = ResourceRelation::find_origin_for_resource(landscape_analysis_id, pool)?;
        let trace_mirrors: Vec<TraceMirror> = origins
            .into_iter()
            .filter(|origin| {
                origin.resource_relation.relation_type == "lnds"
                    && origin.origin_resource.is_trace_mirror()
            })
            .map(|origin| TraceMirror::find_full_trace_mirror(origin.origin_resource.id, pool))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(trace_mirrors)
    }

    /// Get all trace mirrors for a specific trace
    pub fn find_by_trace(trace_id: Uuid, pool: &DbPool) -> Result<Vec<TraceMirror>, PpdcError> {
        let origins = ResourceRelation::find_origin_for_resource(trace_id, pool)?;
        let trace_mirrors: Vec<TraceMirror> = origins
            .into_iter()
            .filter(|origin| {
                origin.resource_relation.relation_type == "trce"
                    && origin.origin_resource.is_trace_mirror()
            })
            .map(|origin| TraceMirror::find_full_trace_mirror(origin.origin_resource.id, pool))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(trace_mirrors)
    }

    /// Get all trace mirrors for a user
    pub fn find_by_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<TraceMirror>, PpdcError> {
        let interactions =
            Interaction::find_paginated_outputs_for_user(0, 1000, user_id, "trcm", pool)?;
        let trace_mirrors: Vec<TraceMirror> = interactions
            .into_iter()
            .map(|interaction| TraceMirror::find_full_trace_mirror(interaction.resource.id, pool))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(trace_mirrors)
    }

    pub fn get_high_level_projects(&self, pool: &DbPool) -> Result<Vec<Landmark>, PpdcError> {
        let mut projects = Vec::new();
        let mut seen = HashSet::new();

        // Source of truth: references from trace mirror -> high level project landmarks.
        let references = Reference::find_for_trace_mirror(self.id, pool)?;
        for reference in references {
            let Some(landmark_id) = reference.landmark_id else {
                continue;
            };
            if !seen.insert(landmark_id) {
                continue;
            }
            let landmark = Landmark::find(landmark_id, pool)?;
            if landmark.landmark_type == LandmarkType::HighLevelProject {
                projects.push(landmark);
            }
        }

        Ok(projects)
    }

    pub fn find_high_level_projects(
        trace_mirror_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Landmark>, PpdcError> {
        let trace_mirror = TraceMirror::find_full_trace_mirror(trace_mirror_id, pool)?;
        trace_mirror.get_high_level_projects(pool)
    }
}

impl NewTraceMirror {
    /// Converts to a NewResource for database insertion.
    pub fn to_new_resource(&self) -> NewResource {
        let tags_json = serde_json::to_string(&self.tags).unwrap_or_else(|_| "[]".to_string());

        NewResource {
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: Some(self.content.clone()),
            resource_type: Some(ResourceType::TraceMirror),
            entity_type: Some(EntityType::TraceMirror),
            maturing_state: Some(MaturingState::Draft),
            publishing_state: Some("drft".to_string()),
            category_id: None,
            is_external: Some(false),
            external_content_url: None,
            comment: Some(tags_json),
            image_url: None,
            resource_subtype: None,
        }
    }
}

impl From<Resource> for TraceMirror {
    fn from(resource: Resource) -> Self {
        TraceMirror::from_resource(resource)
    }
}
