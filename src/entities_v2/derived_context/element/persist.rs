use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    interaction::model::NewInteraction,
    resource_relation::NewResourceRelation,
};

use super::model::{Element, NewElement};

impl Element {
    /// Updates the Element in the database via the Resource.
    pub fn update(self, pool: &DbPool) -> Result<Element, PpdcError> {
        let resource = self.to_resource();
        let updated = resource.update(pool)?;
        Ok(Element {
            title: updated.title,
            subtitle: updated.subtitle,
            content: updated.content,
            updated_at: updated.updated_at,
            ..self
        })
    }
}

impl NewElement {
    /// Creates the underlying Resource, then "ownr" -> analysis, "elmt" -> trace,
    /// and optionally "elmt" -> landmark. Also creates an author interaction ("outp").
    pub fn create(self, pool: &DbPool) -> Result<Element, PpdcError> {
        let user_id = self.user_id;
        let analysis_id = self.analysis_id;
        let trace_id = self.trace_id;
        let trace_mirror_id = self.trace_mirror_id;
        let landmark_id = self.landmark_id;
        let interaction_date = self.interaction_date;

        let new_resource = self.to_new_resource();
        let created = new_resource.create(pool)?;
        let element = Element::from_resource(created);

        NewResourceRelation::create_owned_by_analysis_element(
            element.id,
            analysis_id,
            user_id,
            pool,
        )?;

        NewResourceRelation::create_extracted_from_trace(element.id, trace_id, user_id, pool)?;

        if let Some(trace_mirror_id) = trace_mirror_id {
            NewResourceRelation::create_extracted_in_trace_mirror(
                element.id,
                trace_mirror_id,
                user_id,
                pool,
            )?;
        }

        if let Some(lid) = landmark_id {
            NewResourceRelation::create_involves_landmark(element.id, lid, user_id, pool)?;
        }

        let mut new_interaction = NewInteraction::new(user_id, element.id);
        new_interaction.interaction_type = Some("outp".to_string());
        if let Some(interaction_date) = interaction_date {
            new_interaction.interaction_date = Some(interaction_date);
        }
        new_interaction.create(pool)?;

        Ok(Element {
            interaction_date,
            user_id,
            analysis_id,
            trace_id,
            trace_mirror_id,
            landmark_id,
            ..element
        })
    }
}

pub fn link_to_landmark(
    element_id: Uuid,
    landmark_id: Uuid,
    user_id: Uuid,
    pool: &DbPool,
) -> Result<Uuid, PpdcError> {
    NewResourceRelation::create_involves_landmark(element_id, landmark_id, user_id, pool)?;
    Ok(landmark_id)
}
