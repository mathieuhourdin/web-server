use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    resource::{
        entity_type::EntityType,
        maturing_state::MaturingState,
        NewResource,
        Resource,
        resource_type::ResourceType,
    },
    resource_relation::ResourceRelation,
};

use super::model::{Element, NewElement};

impl Element {
    /// Creates an Element from a Resource. Relations (user_id, analysis_id, trace_id, trace_mirror_id)
    /// and extended_content must be hydrated via `with_*` / `find_full`.
    pub fn from_resource(resource: Resource) -> Self {
        let element_type = resource.resource_type.into();
        let verb = resource.comment.unwrap_or_default();
        Self {
            id: resource.id,
            title: resource.title,
            subtitle: resource.subtitle,
            content: resource.content,
            extended_content: None,
            element_type,
            verb,
            interaction_date: None,
            user_id: Uuid::nil(),
            analysis_id: Uuid::nil(),
            trace_id: Uuid::nil(),
            trace_mirror_id: None,
            landmark_id: None,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }

    /// Converts the Element back to a Resource for writes.
    /// `extended_content` is not persisted (no dedicated Resource field).
    pub fn to_resource(&self) -> Resource {
        Resource {
            id: self.id,
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: self.content.clone(),
            external_content_url: None,
            comment: Some(self.verb.clone()),
            image_url: None,
            resource_type: self.element_type.into(),
            entity_type: EntityType::Element,
            maturing_state: MaturingState::Draft,
            publishing_state: "drft".to_string(),
            category_id: None,
            is_external: false,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// Finds an Element by id (basic retrieval, no hydration).
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Element, PpdcError> {
        let resource = Resource::find(id, pool)?;
        Ok(Element::from_resource(resource))
    }

    /// Hydrates `user_id` from the author interaction ("outp"), when present.
    pub fn with_user_id(self, pool: &DbPool) -> Result<Element, PpdcError> {
        let resource = self.to_resource();
        let interaction = resource.find_resource_author_interaction(pool)?;
        Ok(Element {
            user_id: interaction.interaction_user_id,
            interaction_date: Some(interaction.interaction_date),
            ..self
        })
    }

    /// Hydrates `analysis_id` from the "ownr" relation (element -> analysis).
    pub fn with_analysis_id(self, pool: &DbPool) -> Result<Element, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let analysis_id = targets
            .into_iter()
            .find(|t| t.resource_relation.relation_type == "ownr")
            .map(|t| t.target_resource.id);
        Ok(Element {
            analysis_id: analysis_id.unwrap_or(Uuid::nil()),
            ..self
        })
    }

    /// Hydrates `trace_id` from an "elmt" relation whose target is a Trace.
    pub fn with_trace_id(self, pool: &DbPool) -> Result<Element, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let trace_id = targets
            .into_iter()
            .find(|t| {
                t.resource_relation.relation_type == "elmt"
                    && t.target_resource.is_trace()
            })
            .map(|t| t.target_resource.id);
        Ok(Element {
            trace_id: trace_id.unwrap_or(Uuid::nil()),
            ..self
        })
    }

    /// Hydrates `trace_mirror_id` from a "trcm" relation (element -> trace mirror).
    pub fn with_trace_mirror_id(self, pool: &DbPool) -> Result<Element, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let trace_mirror_id = targets
            .into_iter()
            .find(|t| t.resource_relation.relation_type == "trcm")
            .map(|t| t.target_resource.id);
        Ok(Element {
            trace_mirror_id,
            ..self
        })
    }

    /// Hydrates `landmark_id` from an "elmt" relation whose target is a Landmark (not a Trace).
    pub fn with_landmark_id(self, pool: &DbPool) -> Result<Element, PpdcError> {
        let targets = ResourceRelation::find_target_for_resource(self.id, pool)?;
        let landmark_id = targets
            .into_iter()
            .find(|t| {
                t.resource_relation.relation_type == "elmt"
                    && !t.target_resource.is_trace()
            })
            .map(|t| t.target_resource.id);
        Ok(Element {
            landmark_id,
            ..self
        })
    }

    pub fn with_interaction_date(self, pool: &DbPool) -> Result<Element, PpdcError> {
        let interaction = self.to_resource().find_resource_author_interaction(pool);
        if let Ok(interaction) = interaction {
            Ok(Element {
                interaction_date: Some(interaction.interaction_date),
                ..self
            })
        } else {
            Ok(self)
        }
    }

    /// Finds an Element by id and fully hydrates it (user_id, analysis_id, trace_id, trace_mirror_id, landmark_id).
    pub fn find_full(id: Uuid, pool: &DbPool) -> Result<Element, PpdcError> {
        let resource = Resource::find(id, pool)?;
        let el = Element::from_resource(resource);
        let el = el.with_analysis_id(pool)?;
        let el = el.with_trace_id(pool)?;
        let el = el.with_trace_mirror_id(pool)?;
        let el = el.with_landmark_id(pool)?;
        let el = el.with_interaction_date(pool)?;
        let el = match el.clone().with_user_id(pool) {
            Ok(e) => e,
            Err(_) => el,
        };
        Ok(el)
    }

    /// Finds elements linked to a landmark via "elmt" (element -> landmark).
    pub fn find_for_landmark(landmark_id: Uuid, pool: &DbPool) -> Result<Vec<Element>, PpdcError> {
        let rels = ResourceRelation::find_origin_for_resource(landmark_id, pool)?;
        let elements = rels
            .into_iter()
            .filter(|r| {
                r.resource_relation.relation_type == "elmt"
                    && r.origin_resource.resource_type == ResourceType::Event
            })
            .map(|r| Element::from_resource(r.origin_resource))
            .collect();
        Ok(elements)
    }

    /// Finds elements linked to a trace via "elmt" (element -> trace).
    pub fn find_for_trace(trace_id: Uuid, pool: &DbPool) -> Result<Vec<Element>, PpdcError> {
        let rels = ResourceRelation::find_origin_for_resource(trace_id, pool)?;
        let elements = rels
            .into_iter()
            .filter(|r| {
                r.resource_relation.relation_type == "elmt"
                    && r.origin_resource.resource_type == ResourceType::Event
            })
            .map(|r| Element::from_resource(r.origin_resource))
            .collect();
        Ok(elements)
    }
}

impl NewElement {
    pub fn to_new_resource(&self) -> NewResource {
        NewResource {
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: Some(self.content.clone()),
            external_content_url: None,
            comment: Some(self.verb.clone()),
            image_url: None,
            resource_type: Some(self.element_type.into()),
            entity_type: Some(EntityType::Element),
            maturing_state: Some(MaturingState::Draft),
            publishing_state: Some("drft".to_string()),
            category_id: None,
            is_external: Some(false),
        }
    }
}

impl From<Resource> for Element {
    fn from(resource: Resource) -> Self {
        Element::from_resource(resource)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime};

    use crate::entities::resource::{
        entity_type::EntityType,
        maturing_state::MaturingState,
        resource_type::ResourceType,
        Resource,
    };

    use super::{Element, NewElement};
    use crate::entities_v2::element::ElementType;

    fn sample_datetime() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 1, 1)
            .expect("valid date")
            .and_hms_opt(0, 0, 0)
            .expect("valid time")
    }

    #[test]
    fn to_new_resource_persists_verb_in_comment() {
        let new_element = NewElement::new(
            "Title".to_string(),
            "Subtitle".to_string(),
            "Content".to_string(),
            ElementType::Event,
            "écouter".to_string(),
            None,
            uuid::Uuid::nil(),
            None,
            None,
            uuid::Uuid::nil(),
            uuid::Uuid::nil(),
        );

        let new_resource = new_element.to_new_resource();
        assert_eq!(new_resource.comment, Some("écouter".to_string()));
    }

    #[test]
    fn from_resource_keeps_legacy_comment_raw_as_verb() {
        let resource = Resource {
            id: uuid::Uuid::nil(),
            title: "Title".to_string(),
            subtitle: "Subtitle".to_string(),
            content: "Content".to_string(),
            external_content_url: None,
            comment: Some("evnt".to_string()),
            image_url: None,
            resource_type: ResourceType::Event,
            maturing_state: MaturingState::Draft,
            publishing_state: "drft".to_string(),
            category_id: None,
            is_external: false,
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
            entity_type: EntityType::Element,
        };

        let element = Element::from_resource(resource);
        assert_eq!(element.verb, "evnt");
        assert_eq!(element.element_type, ElementType::Event);
    }
}
