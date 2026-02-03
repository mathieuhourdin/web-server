use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::{Interaction, NewInteraction},
    resource::{
        entity_type::EntityType,
        maturing_state::MaturingState,
        resource_type::ResourceType,
        NewResource,
        Resource,
    },
    resource_relation::{NewResourceRelation, ResourceRelation},
    session::Session,
};
use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TraceMirror {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub tags: Vec<String>,
    pub trace_id: Uuid,
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub primary_resource_id: Option<Uuid>,
    pub primary_theme_id: Option<Uuid>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

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
        Ok(TraceMirror {
            trace_id,
            ..self
        })
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
        let interactions = Interaction::find_paginated_outputs_for_user(
            0,
            1000,
            user_id,
            "trcm",
            pool,
        )?;
        let trace_mirrors: Vec<TraceMirror> = interactions
            .into_iter()
            .map(|interaction| TraceMirror::find_full_trace_mirror(interaction.resource.id, pool))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(trace_mirrors)
    }
}

/// Struct for creating a new TraceMirror with all required data.
pub struct NewTraceMirror {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub tags: Vec<String>,
    pub trace_id: Uuid,
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub primary_resource_id: Option<Uuid>,
    pub primary_theme_id: Option<Uuid>,
    pub interaction_date: Option<NaiveDateTime>,
}

impl NewTraceMirror {
    /// Creates a new NewTraceMirror with all fields.
    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        tags: Vec<String>,
        trace_id: Uuid,
        landscape_analysis_id: Uuid,
        user_id: Uuid,
        primary_resource_id: Option<Uuid>,
        primary_theme_id: Option<Uuid>,
        interaction_date: Option<NaiveDateTime>,
    ) -> NewTraceMirror {
        NewTraceMirror {
            title,
            subtitle,
            content,
            tags,
            trace_id,
            landscape_analysis_id,
            user_id,
            primary_resource_id,
            primary_theme_id,
            interaction_date,
        }
    }

    /// Converts to a NewResource for database insertion.
    fn to_new_resource(&self) -> NewResource {
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
        }
    }

    /// Creates the TraceMirror in the database.
    /// This creates the underlying Resource, Interaction, and ResourceRelations.
    pub fn create(self, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let user_id = self.user_id;
        let interaction_date = self.interaction_date.unwrap_or_else(|| Utc::now().naive_utc());
        let trace_id = self.trace_id;
        let landscape_analysis_id = self.landscape_analysis_id;
        let primary_resource_id = self.primary_resource_id;
        let primary_theme_id = self.primary_theme_id;

        // Create the underlying resource
        let new_resource = self.to_new_resource();
        let created_resource = new_resource.create(pool)?;

        // Create the author interaction
        let mut new_interaction = NewInteraction::new(user_id, created_resource.id);
        new_interaction.interaction_type = Some("outp".to_string());
        new_interaction.interaction_date = Some(interaction_date);
        new_interaction.interaction_progress = 0;
        new_interaction.create(pool)?;

        // Create trace relation
        let mut trace_relation = NewResourceRelation::new(created_resource.id, trace_id);
        trace_relation.relation_type = Some("trce".to_string());
        trace_relation.user_id = Some(user_id);
        trace_relation.create(pool)?;

        // Create landscape analysis relation
        let mut landscape_relation =
            NewResourceRelation::new(created_resource.id, landscape_analysis_id);
        landscape_relation.relation_type = Some("lnds".to_string());
        landscape_relation.user_id = Some(user_id);
        landscape_relation.create(pool)?;

        // Create primary resource relation if provided
        if let Some(resource_id) = primary_resource_id {
            let mut resource_relation = NewResourceRelation::new(created_resource.id, resource_id);
            resource_relation.relation_type = Some("prir".to_string());
            resource_relation.user_id = Some(user_id);
            resource_relation.create(pool)?;
        }

        // Create primary theme relation if provided
        if let Some(theme_id) = primary_theme_id {
            let mut theme_relation = NewResourceRelation::new(created_resource.id, theme_id);
            theme_relation.relation_type = Some("prit".to_string());
            theme_relation.user_id = Some(user_id);
            theme_relation.create(pool)?;
        }

        // Return the fully hydrated trace mirror
        TraceMirror::find_full_trace_mirror(created_resource.id, pool)
    }
}

#[debug_handler]
pub async fn get_trace_mirror_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<TraceMirror>, PpdcError> {
    let trace_mirror = TraceMirror::find_full_trace_mirror(id, &pool)?;
    Ok(Json(trace_mirror))
}

#[debug_handler]
pub async fn get_trace_mirrors_by_landscape_route(
    Extension(pool): Extension<DbPool>,
    Path(landscape_analysis_id): Path<Uuid>,
) -> Result<Json<Vec<TraceMirror>>, PpdcError> {
    let trace_mirrors = TraceMirror::find_by_landscape_analysis(landscape_analysis_id, &pool)?;
    Ok(Json(trace_mirrors))
}

#[debug_handler]
pub async fn get_trace_mirrors_by_trace_route(
    Extension(pool): Extension<DbPool>,
    Path(trace_id): Path<Uuid>,
) -> Result<Json<Vec<TraceMirror>>, PpdcError> {
    let trace_mirrors = TraceMirror::find_by_trace(trace_id, &pool)?;
    Ok(Json(trace_mirrors))
}

/// Links a trace mirror to its primary resource landmark
pub fn link_to_primary_resource(trace_mirror_id: Uuid, primary_resource_id: Uuid, user_id: Uuid, pool: &DbPool) -> Result<Uuid, PpdcError> {
    let mut mirror_resource = NewResourceRelation::new(trace_mirror_id, primary_resource_id);
    mirror_resource.relation_type = Some("prir".to_string());
    mirror_resource.user_id = Some(user_id);
    mirror_resource.create(pool)?;
    Ok(primary_resource_id)
}

#[debug_handler]
pub async fn get_user_trace_mirrors_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<TraceMirror>>, PpdcError> {
    let trace_mirrors = TraceMirror::find_by_user(session.user_id.unwrap(), &pool)?;
    Ok(Json(trace_mirrors))
}
