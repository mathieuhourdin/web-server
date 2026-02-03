use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    interaction::model::NewInteraction,
    resource_relation::{NewResourceRelation, ResourceRelation},
};
use crate::entities_v2::{
    landscape_analysis::{self, LandscapeAnalysis, NewLandscapeAnalysis},
    trace::Trace,
};

use super::model::{Lens, NewLens};

impl Lens {
    pub fn delete(self, pool: &DbPool) -> Result<Lens, PpdcError> {
        let resource = self.to_resource();
        let deleted_resource = resource.delete(pool)?;
        Ok(Lens::from_resource(deleted_resource))
    }

    pub fn update_current_landscape(self, new_landscape_analysis_id: Uuid, pool: &DbPool) -> Result<Lens, PpdcError> {
        // Get the new landscape to retrieve its trace_id
        let new_landscape = LandscapeAnalysis::find_full_analysis(new_landscape_analysis_id, pool)?;
        let new_trace_id = new_landscape.analyzed_trace_id;

        let relations = ResourceRelation::find_target_for_resource(self.id, pool)?;
        
        // Find and separate the landscape and trace relations
        let mut current_landscape_relation = None;
        let mut current_trace_relation = None;
        
        for relation in relations {
            match relation.resource_relation.relation_type.as_str() {
                "head" => current_landscape_relation = Some(relation.resource_relation),
                "trgt" => current_trace_relation = Some(relation.resource_relation),
                _ => {}
            }
        }

        // Update landscape relation (head)
        if current_landscape_relation.is_none() {
            return Err(PpdcError::new(404, ErrorType::ApiError, "Current landscape not found".to_string()));
        }
        current_landscape_relation.unwrap().delete(pool)?;
        let mut new_current_landscape_relation = NewResourceRelation::new(self.id, new_landscape_analysis_id);
        new_current_landscape_relation.relation_type = Some("head".to_string());
        new_current_landscape_relation.user_id = Some(self.user_id.unwrap());
        new_current_landscape_relation.create(pool)?;

        // Update trace relation (trgt)
        // Delete existing trace relation if it exists
        if let Some(trace_relation) = current_trace_relation {
            trace_relation.delete(pool)?;
        }
        
        // Only create new trace relation if the new landscape has an analyzed trace
        if let Some(trace_id) = new_trace_id {
            let mut new_current_trace_relation = NewResourceRelation::new(self.id, trace_id);
            new_current_trace_relation.relation_type = Some("trgt".to_string());
            new_current_trace_relation.user_id = Some(self.user_id.unwrap());
            new_current_trace_relation.create(pool)?;
        }

        Ok(self)
    }
}

impl NewLens {
    pub fn create(self, pool: &DbPool) -> Result<Lens, PpdcError> {

        let user_id = self.user_id;
        let current_state_date = self.current_state_date;
        let current_trace_id = self.current_trace_id;
        let fork_landscape_id = self.fork_landscape_id;
        let current_landscape_id = self.current_landscape_id;

        let new_resource = self.to_new_resource();
        let created_resource = new_resource.create(pool)?;
        let mut new_interaction = NewInteraction::new(user_id, created_resource.id);
        new_interaction.interaction_type = Some("outp".to_string());
        new_interaction.interaction_date = Some(current_state_date);
        new_interaction.interaction_progress = 0;
        new_interaction.create(pool)?;
        if let Some(fork_landscape_id) = fork_landscape_id {
            let mut new_fork_relation = NewResourceRelation::new(created_resource.id, fork_landscape_id);
            new_fork_relation.relation_type = Some("fork".to_string());
            new_fork_relation.user_id = Some(user_id);
            new_fork_relation.create(pool)?;
        }
        let mut new_landscape_relation = NewResourceRelation::new(created_resource.id, current_landscape_id);
        new_landscape_relation.relation_type = Some("head".to_string());
        new_landscape_relation.user_id = Some(user_id);
        new_landscape_relation.create(pool)?;
        let mut new_trace_relation = NewResourceRelation::new(created_resource.id, current_trace_id);
        new_trace_relation.relation_type = Some("trgt".to_string());
        new_trace_relation.user_id = Some(user_id);
        new_trace_relation.create(pool)?;
        let lens = Lens::from_resource(created_resource);
        Ok(lens)
    }
}

pub fn create_landscape_placeholders(current_trace_id: Uuid, previous_landscape_analysis_id: Uuid, pool: &DbPool) -> Result<Vec<Uuid>, PpdcError> {
    let previous_landscape = LandscapeAnalysis::find_full_analysis(previous_landscape_analysis_id, pool)?;
    let user_id = previous_landscape.user_id;
    let traces: Vec<Trace>;
    if previous_landscape.analyzed_trace_id.is_none() {
        traces = Trace::get_before(user_id, current_trace_id, pool)?;
    } else {
        traces = Trace::get_between(user_id, previous_landscape.analyzed_trace_id.unwrap(), current_trace_id, pool)?;
    }
    let mut landscape_analysis_ids = vec![previous_landscape_analysis_id];
    let mut previous_id = previous_landscape_analysis_id;
    for trace in traces {
        let new_landscape_analysis = NewLandscapeAnalysis::new_placeholder(
            user_id,
            trace.id,
            Some(previous_id),
        ).create(pool)?;
        landscape_analysis_ids.push(new_landscape_analysis.id);
        previous_id = new_landscape_analysis.id;
    }
    Ok(landscape_analysis_ids)
}

pub fn delete_lens_and_landscapes(id: Uuid, pool: &DbPool) -> Result<Lens, PpdcError> {
    println!("Deleting lens: {:?}", id);
    let mut lens = Lens::find_full_lens(id, pool)?;
    let mut landscape_analysis_id = lens.current_landscape_id;

    while let Some(id) = landscape_analysis_id {
        println!("Deleting landscape analysis: {:?}", id);
        let current_landscape_analysis = LandscapeAnalysis::find_full_analysis(id, pool)?;
        println!("Current landscape analysis: {:?}", current_landscape_analysis);
        if let Some(parent_analysis_id) = current_landscape_analysis.parent_analysis_id {
            // update head to the parent landscape_analysis before to delete the current landscape_analysis
            println!("Updating head to parent landscape analysis: {:?}", parent_analysis_id);
            lens = lens.update_current_landscape(parent_analysis_id, pool)?;
        } else {
            // we reached last landscape_analysis, delete the lens before deleting the root landscape_analysis
            println!("Deleting lens");
            lens = lens.delete(pool)?;
            landscape_analysis::delete_leaf_and_cleanup(current_landscape_analysis.id, pool)?;
            return Ok(lens);
        }
        println!("Deleting landscape analysis: {:?}", current_landscape_analysis.id);
        let deletion_option = landscape_analysis::delete_leaf_and_cleanup(current_landscape_analysis.id, pool)?;
        println!("Deletion option: {:?}", deletion_option);
        if let Some(_deletion_option) = deletion_option {
            // deletion succeded, continue with the parent.
            landscape_analysis_id = current_landscape_analysis.parent_analysis_id;

        } else {
            // we reached a landscape analysis referenced by other lens or landscape_analysis, just delete the lens
            println!("Reached a landscape analysis referenced by other lens or landscape_analysis, just deleting the lens");
            break;
        }
    }
    println!("Deleting lens: {:?}", id);
    let lens = lens.delete(pool)?;
    println!("Lens deleted: {:?}", lens);
    Ok(lens)
}
