use chrono::Utc;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError, interaction::model::NewInteraction, resource::NewResource,
    resource_relation::NewResourceRelation,
};

use super::model::{NewTraceMirror, TraceMirror};

impl TraceMirror {
    /// Updates the TraceMirror resource fields in the database.
    pub fn update(self, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let resource = self.to_resource();
        let _updated_resource = NewResource::from(resource).update(&self.id, pool)?;
        TraceMirror::find_full_trace_mirror(self.id, pool)
    }
}

impl NewTraceMirror {
    /// Creates the TraceMirror in the database.
    /// This creates the underlying Resource, Interaction, and ResourceRelations.
    pub fn create(self, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let user_id = self.user_id;
        let interaction_date = self
            .interaction_date
            .unwrap_or_else(|| Utc::now().naive_utc());
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

/// Links a trace mirror to its primary resource landmark
pub fn link_to_primary_resource(
    trace_mirror_id: Uuid,
    primary_resource_id: Uuid,
    user_id: Uuid,
    pool: &DbPool,
) -> Result<Uuid, PpdcError> {
    let mut mirror_resource = NewResourceRelation::new(trace_mirror_id, primary_resource_id);
    mirror_resource.relation_type = Some("prir".to_string());
    mirror_resource.user_id = Some(user_id);
    mirror_resource.create(pool)?;
    Ok(primary_resource_id)
}
