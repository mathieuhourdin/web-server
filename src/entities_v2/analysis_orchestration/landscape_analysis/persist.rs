use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::{Interaction, InteractionWithResource, NewInteraction},
    resource::{maturing_state::MaturingState, NewResource},
    resource_relation::NewResourceRelation,
};
use crate::entities_v2::reference::Reference;

use super::model::{LandscapeAnalysis, NewLandscapeAnalysis};

impl LandscapeAnalysis {
    /// Updates the LandscapeAnalysis in the database.
    /// Only updates the underlying Resource fields (title, subtitle, content, maturing_state).
    pub fn update(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let resource = self.to_resource();
        let updated_resource = NewResource::from(resource).update(&self.id, pool)?;
        Ok(LandscapeAnalysis {
            title: updated_resource.title,
            subtitle: updated_resource.subtitle,
            plain_text_state_summary: updated_resource.content,
            processing_state: updated_resource.maturing_state,
            updated_at: updated_resource.updated_at,
            ..self
        })
    }

    /// Updates the processing state of the analysis.
    pub fn set_processing_state(
        mut self,
        state: MaturingState,
        pool: &DbPool,
    ) -> Result<LandscapeAnalysis, PpdcError> {
        self.processing_state = state;
        self.update(pool)
    }

    pub fn delete(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let resource = self.to_resource();
        let deleted_resource = resource.delete(pool)?;
        Ok(LandscapeAnalysis::from_resource(deleted_resource))
    }

    /*pub fn replay(self, lens_id: Uuid, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let analysis = LandscapeAnalysis::find_full_analysis(self.id, pool)?;

        if analysis.processing_state != MaturingState::Finished {
            return Err(PpdcError::new(
                409,
                ErrorType::ApiError,
                "Replay is only allowed for finished analyses".to_string(),
            ));
        }

        let children = analysis.get_children_landscape_analyses(pool)?;
        if !children.is_empty() {
            return Err(PpdcError::new(
                409,
                ErrorType::ApiError,
                "Replay rejected: analysis has children".to_string(),
            ));
        }

        let trace_id = analysis.analyzed_trace_id.ok_or_else(|| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                "Replay requires an analyzed trace id".to_string(),
            )
        })?;

        let user_id = analysis.user_id;
        let parent_analysis_id = analysis.parent_analysis_id;
        let old_analysis_id = analysis.id;

        // Use processing_state as a lightweight lock
        let _trashed = analysis
            .clone()
            .set_processing_state(MaturingState::Trashed, pool)?;

        // Re-check children to reduce race risks (best-effort, no locks)
        let children_after = analysis.get_children_landscape_analyses(pool)?;
        if !children_after.is_empty() {
            let _ = analysis.clone().set_processing_state(MaturingState::Finished, pool);
            return Err(PpdcError::new(
                409,
                ErrorType::ApiError,
                "Replay rejected: analysis got children during replay".to_string(),
            ));
        }

        let new_analysis = match NewLandscapeAnalysis::new_placeholder(
            user_id,
            trace_id,
            parent_analysis_id,
            replayed_from_id,
        );
        let new_analysis = new_analysis.create(pool)?;

        let mut replay_relation = NewResourceRelation::new(new_analysis.id, old_analysis_id);
        replay_relation.relation_type = Some("rply".to_string());
        replay_relation.user_id = Some(user_id);
        replay_relation.create(pool)?;

        let lenses = analysis.get_heading_lens(pool)?;
        for lens in lenses {
            lens.update_current_landscape(new_analysis.id, pool)?;
        }

        Ok(new_analysis)
    }*/
}

impl NewLandscapeAnalysis {
    /// Creates the LandscapeAnalysis in the database.
    /// This creates the underlying Resource, Interaction, and ResourceRelations.
    pub fn create(self, pool: &DbPool) -> Result<LandscapeAnalysis, PpdcError> {
        let user_id = self.user_id;
        let interaction_date = self.interaction_date;
        let parent_analysis_id = self.parent_analysis_id;
        let analyzed_trace_id = self.analyzed_trace_id;
        let replayed_from_id = self.replayed_from_id;
        let trace_mirror_id = self.trace_mirror_id;

        // Create the underlying resource
        let new_resource = self.to_new_resource();
        let created_resource = new_resource.create(pool)?;

        // Create the author interaction
        let mut new_interaction = NewInteraction::new(user_id, created_resource.id);
        new_interaction.interaction_type = Some("outp".to_string());
        new_interaction.interaction_date = Some(interaction_date);
        new_interaction.interaction_progress = 0;
        new_interaction.create(pool)?;

        // Create parent analysis relation if provided
        if let Some(parent_id) = parent_analysis_id {
            let mut parent_relation = NewResourceRelation::new(created_resource.id, parent_id);
            parent_relation.relation_type = Some("prnt".to_string());
            parent_relation.user_id = Some(user_id);
            parent_relation.create(pool)?;
        }

        // Create analyzed trace relation if provided
        if let Some(trace_id) = analyzed_trace_id {
            let mut trace_relation = NewResourceRelation::new(created_resource.id, trace_id);
            trace_relation.relation_type = Some("trce".to_string());
            trace_relation.user_id = Some(user_id);
            trace_relation.create(pool)?;
        }
        if let Some(replayed_from_id) = replayed_from_id {
            let mut replayed_from_relation =
                NewResourceRelation::new(created_resource.id, replayed_from_id);
            replayed_from_relation.relation_type = Some("rply".to_string());
            replayed_from_relation.user_id = Some(user_id);
            replayed_from_relation.create(pool)?;
        }
        if let Some(trace_mirror_id) = trace_mirror_id {
            let mut trace_mirror_relation =
                NewResourceRelation::new(trace_mirror_id, created_resource.id);
            trace_mirror_relation.relation_type = Some("lnds".to_string());
            trace_mirror_relation.user_id = Some(user_id);
            trace_mirror_relation.create(pool)?;
        }

        // Return the fully hydrated analysis
        LandscapeAnalysis::find_full_analysis(created_resource.id, pool)
    }
}

pub fn delete_leaf_and_cleanup(
    id: Uuid,
    pool: &DbPool,
) -> Result<Option<LandscapeAnalysis>, PpdcError> {
    println!("Deleting leaf and cleanup for analysis: {:?}", id);
    let landscape_analysis = LandscapeAnalysis::find_full_analysis(id, pool)?;
    println!("Landscape analysis: {:?}", landscape_analysis);
    let children_landscape_analyses = landscape_analysis.get_children_landscape_analyses(pool)?;
    println!(
        "Children landscape analyses: {:?}",
        children_landscape_analyses
    );
    let heading_lens = landscape_analysis.get_heading_lens(pool)?;
    println!("Heading lens: {:?}", heading_lens);
    if children_landscape_analyses.len() > 0 || heading_lens.len() > 0 {
        println!("Children landscape analyses or heading lens found, not deleting");
        return Ok(None);
    }
    Reference::delete_for_landscape_analysis(id, pool)?;
    println!("Analysis: {:?}", landscape_analysis);
    let related_resources =
        crate::entities::resource_relation::ResourceRelation::find_origin_for_resource(id, pool)?;
    println!("Related resources: {:?}", related_resources);
    for resource_relation in related_resources {
        println!("Resource relation: {:?}", resource_relation);
        if resource_relation.origin_resource.is_trace_mirror() {
            println!(
                "Deleting trace mirror: {:?}",
                resource_relation.origin_resource
            );
            resource_relation.origin_resource.delete(pool)?;
            continue;
        }
        // check if the resource is owned by the analysis (for elements and entities)
        if resource_relation.resource_relation.relation_type == "ownr".to_string()
            && !resource_relation.origin_resource.is_trace()
            && !resource_relation.origin_resource.is_lens()
            && !resource_relation.origin_resource.is_landscape_analysis()
        {
            println!("Deleting resource: {:?}", resource_relation.origin_resource);
            resource_relation.origin_resource.delete(pool)?;
        } else if resource_relation.origin_resource.is_trace() {
            let mut trace = resource_relation.origin_resource;
            trace.maturing_state = MaturingState::Draft;
            trace.update(pool)?;
        }
    }
    println!("Deleting landscape analysis: {:?}", landscape_analysis);
    let landscape_analysis = landscape_analysis.delete(pool)?;
    Ok(Some(landscape_analysis))
}

pub fn find_last_analysis_resource(
    user_id: Uuid,
    pool: &DbPool,
) -> Result<Option<InteractionWithResource>, PpdcError> {
    let interaction = Interaction::find_last_analysis_for_user(user_id, pool)?;
    Ok(interaction)
}

pub fn add_landmark_ref(
    landscape_analysis_id: Uuid,
    landmark_id: Uuid,
    user_id: Uuid,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut new_resource_relation = NewResourceRelation::new(landmark_id, landscape_analysis_id);
    new_resource_relation.relation_type = Some("refr".to_string());
    new_resource_relation.user_id = Some(user_id);
    new_resource_relation.create(pool)?;
    Ok(())
}
