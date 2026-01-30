use uuid::Uuid;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::entities::{
    error::PpdcError,
    resource::{
        Resource, NewResource,
        resource_type::ResourceType,
        maturing_state::MaturingState,
    },
    resource_relation::{NewResourceRelation, ResourceRelation},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Element {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub extended_content: Option<String>,
    pub element_type: ElementType,
    pub user_id: Uuid,
    pub analysis_id: Uuid,
    pub trace_id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum ElementType {
    TraceMirror,
    Action,
    Event,
    Idea,
    Quote,
    Emotion,
    Fact,
}

impl ElementType {
    pub fn to_code(self) -> &'static str {
        match self {
            ElementType::TraceMirror => "trcm",
            ElementType::Action => "actn",
            ElementType::Event => "evnt",
            ElementType::Idea => "idea",
            ElementType::Quote => "quot",
            ElementType::Emotion => "emot",
            ElementType::Fact => "fact",
        }
    }

    pub fn from_code(code: &str) -> Option<ElementType> {
        match code {
            "trcm" => Some(ElementType::TraceMirror),
            "actn" => Some(ElementType::Action),
            "evnt" => Some(ElementType::Event),
            "idea" => Some(ElementType::Idea),
            "quot" => Some(ElementType::Quote),
            "emot" => Some(ElementType::Emotion),
            "fact" => Some(ElementType::Fact),
            _ => None,
        }
    }
}

impl From<ResourceType> for ElementType {
    fn from(resource_type: ResourceType) -> Self {
        match resource_type {
            ResourceType::TraceMirror => ElementType::TraceMirror,
            _ => ElementType::Event,
        }
    }
}

impl From<ElementType> for ResourceType {
    fn from(element_type: ElementType) -> Self {
        match element_type {
            ElementType::TraceMirror => ResourceType::TraceMirror,
            _ => ResourceType::Event,
        }
    }
}
impl Element {
    /// Creates an Element from a Resource. Relations (user_id, analysis_id, trace_id)
    /// and extended_content must be hydrated via `with_*` / `find_full`.
    pub fn from_resource(resource: Resource) -> Self {
        let element_type = resource
            .comment
            .as_deref()
            .and_then(ElementType::from_code)
            .unwrap_or(ElementType::Event);
        Self {
            id: resource.id,
            title: resource.title,
            subtitle: resource.subtitle,
            content: resource.content,
            extended_content: None,
            element_type: element_type.into(),
            user_id: Uuid::nil(),
            analysis_id: Uuid::nil(),
            trace_id: Uuid::nil(),
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
            comment: Some(self.element_type.to_code().to_string()),
            image_url: None,
            resource_type: self.element_type.into(),
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
                    && t.target_resource.resource_type == ResourceType::Trace
            })
            .map(|t| t.target_resource.id);
        Ok(Element {
            trace_id: trace_id.unwrap_or(Uuid::nil()),
            ..self
        })
    }

    /// Finds an Element by id and fully hydrates it (user_id, analysis_id, trace_id).
    pub fn find_full(id: Uuid, pool: &DbPool) -> Result<Element, PpdcError> {
        let resource = Resource::find(id, pool)?;
        let el = Element::from_resource(resource);
        let el = el.with_analysis_id(pool)?;
        let el = el.with_trace_id(pool)?;
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

impl From<Resource> for Element {
    fn from(resource: Resource) -> Self {
        Element::from_resource(resource)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewElement {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub element_type: ElementType,
    pub user_id: Uuid,
    pub analysis_id: Uuid,
    pub trace_id: Uuid,
    pub landmark_id: Option<Uuid>,
}

impl NewElement {
    pub fn to_new_resource(&self) -> NewResource {
        NewResource {
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            content: Some(self.content.clone()),
            external_content_url: None,
            comment: Some(self.element_type.to_code().to_string()),
            image_url: None,
            resource_type: Some(self.element_type.into()),
            maturing_state: Some(MaturingState::Draft),
            publishing_state: Some("drft".to_string()),
            category_id: None,
            is_external: Some(false),
        }
    }

    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        element_type: ElementType,
        trace_id: Uuid,
        landmark_id: Option<Uuid>,
        analysis_id: Uuid,
        user_id: Uuid,
    ) -> NewElement {
        NewElement { title, subtitle, content, element_type, user_id, analysis_id, trace_id, landmark_id }
    }

    /// Creates the underlying Resource, then "ownr" -> analysis, "elmt" -> trace,
    /// and optionally "elmt" -> landmark. Does not create an interaction.
    pub fn create(self, pool: &DbPool) -> Result<Element, PpdcError> {
        let user_id = self.user_id;
        let analysis_id = self.analysis_id;
        let trace_id = self.trace_id;
        let landmark_id = self.landmark_id;

        let new_resource = self.to_new_resource();
        let created = new_resource.create(pool)?;
        let element = Element::from_resource(created);

        let mut ownr = NewResourceRelation::new(element.id, analysis_id);
        ownr.relation_type = Some("ownr".to_string());
        ownr.user_id = Some(user_id);
        ownr.create(pool)?;

        let mut elmt_trace = NewResourceRelation::new(element.id, trace_id);
        elmt_trace.relation_type = Some("elmt".to_string());
        elmt_trace.user_id = Some(user_id);
        elmt_trace.create(pool)?;

        if let Some(lid) = landmark_id {
            let mut elmt_landmark = NewResourceRelation::new(element.id, lid);
            elmt_landmark.relation_type = Some("elmt".to_string());
            elmt_landmark.user_id = Some(user_id);
            elmt_landmark.create(pool)?;
        }

        Ok(Element {
            user_id,
            analysis_id,
            trace_id,
            ..element
        })
    }
}
