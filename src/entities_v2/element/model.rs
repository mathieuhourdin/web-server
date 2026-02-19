use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::resource::resource_type::ResourceType;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Element {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub extended_content: Option<String>,
    pub element_type: ElementType,
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum ElementType {
    TraceMirror,
    TransactionInput,
    TransactionOutput,
    TransactionTransformation,
    TransactionQuestion,
    DescriptiveUnit,
    DescriptiveQuestion,
    DescriptiveTheme,
    NormativePlan,
    NormativeObligation,
    NormativeRecommendation,
    NormativePrinciple,
    EvaluativeEmotion,
    EvaluativeEnergy,
    EvaluativeQuality,
    EvaluativeInterest,
}

impl ElementType {
    pub fn to_code(self) -> &'static str {
        match self {
            ElementType::TraceMirror => "trcm",
            ElementType::TransactionInput => "trin",
            ElementType::TransactionOutput => "trou",
            ElementType::TransactionTransformation => "trtf",
            ElementType::TransactionQuestion => "trqs",
            ElementType::DescriptiveUnit => "dsun",
            ElementType::DescriptiveQuestion => "dsqs",
            ElementType::DescriptiveTheme => "dsth",
            ElementType::NormativePlan => "nrpl",
            ElementType::NormativeObligation => "nrob",
            ElementType::NormativeRecommendation => "nrrc",
            ElementType::NormativePrinciple => "nrpr",
            ElementType::EvaluativeEmotion => "evem",
            ElementType::EvaluativeEnergy => "even",
            ElementType::EvaluativeQuality => "evql",
            ElementType::EvaluativeInterest => "evin",
        }
    }

    pub fn from_code(code: &str) -> Option<ElementType> {
        match code {
            "trcm" => Some(ElementType::TraceMirror),
            "trin" => Some(ElementType::TransactionInput),
            "trou" => Some(ElementType::TransactionOutput),
            "trtf" => Some(ElementType::TransactionTransformation),
            "trqs" => Some(ElementType::TransactionQuestion),
            "dsun" => Some(ElementType::DescriptiveUnit),
            "dsqs" => Some(ElementType::DescriptiveQuestion),
            "dsth" => Some(ElementType::DescriptiveTheme),
            "nrpl" => Some(ElementType::NormativePlan),
            "nrob" => Some(ElementType::NormativeObligation),
            "nrrc" => Some(ElementType::NormativeRecommendation),
            "nrpr" => Some(ElementType::NormativePrinciple),
            "evem" => Some(ElementType::EvaluativeEmotion),
            "even" => Some(ElementType::EvaluativeEnergy),
            "evql" => Some(ElementType::EvaluativeQuality),
            "evin" => Some(ElementType::EvaluativeInterest),
            _ => None,
        }
    }
}

impl From<ResourceType> for ElementType {
    fn from(resource_type: ResourceType) -> Self {
        match resource_type {
            ResourceType::TraceMirror => ElementType::TraceMirror,
            _ => ElementType::TransactionOutput,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct NewElement {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub element_type: ElementType,
    pub verb: String,
    pub interaction_date: Option<NaiveDateTime>,
    pub user_id: Uuid,
    pub analysis_id: Uuid,
    pub trace_id: Uuid,
    pub trace_mirror_id: Option<Uuid>,
    pub landmark_id: Option<Uuid>,
}

impl NewElement {
    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        element_type: ElementType,
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
