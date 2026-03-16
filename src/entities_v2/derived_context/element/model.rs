use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::enums::{ElementStatus, ElementSubtype, ElementType};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Element {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub extended_content: Option<String>,
    pub element_type: ElementType,
    pub element_subtype: ElementSubtype,
    pub status: Option<ElementStatus>,
    pub verb: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub user_id: Uuid,
    pub analysis_id: Uuid,
    pub trace_id: Uuid,
    pub trace_mirror_id: Option<Uuid>,
    pub landmark_id: Option<Uuid>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ElementRelationWithRelatedElement {
    pub relation_type: String,
    pub related_element: Element,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewElement {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub element_type: ElementType,
    pub element_subtype: ElementSubtype,
    pub status: Option<ElementStatus>,
    pub verb: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub user_id: Uuid,
    pub analysis_id: Uuid,
    pub trace_id: Uuid,
    pub trace_mirror_id: Option<Uuid>,
    pub landmark_id: Option<Uuid>,
}

impl NewElement {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        element_type: ElementType,
        element_subtype: ElementSubtype,
        status: Option<ElementStatus>,
        verb: String,
        interaction_date: Option<NaiveDateTime>,
        trace_id: Uuid,
        trace_mirror_id: Option<Uuid>,
        landmark_id: Option<Uuid>,
        analysis_id: Uuid,
        user_id: Uuid,
    ) -> NewElement {
        NewElement {
            title,
            subtitle,
            content,
            element_type,
            element_subtype,
            status,
            verb,
            interaction_date,
            user_id,
            analysis_id,
            trace_id,
            trace_mirror_id,
            landmark_id,
        }
    }
}
