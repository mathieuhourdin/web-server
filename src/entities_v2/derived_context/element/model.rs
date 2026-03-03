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
    pub element_subtype: ElementSubtype,
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    Transaction,
    Descriptive,
    Normative,
    Evaluative,
}

impl ElementType {
    pub fn to_code(self) -> &'static str {
        match self {
            ElementType::Transaction => "tran",
            ElementType::Descriptive => "desc",
            ElementType::Normative => "norm",
            ElementType::Evaluative => "eval",
        }
    }

    pub fn from_code(code: &str) -> Option<ElementType> {
        match code {
            "tran" => Some(ElementType::Transaction),
            "desc" => Some(ElementType::Descriptive),
            "norm" => Some(ElementType::Normative),
            "eval" => Some(ElementType::Evaluative),
            _ => None,
        }
    }

    pub fn to_db(self) -> &'static str {
        match self {
            ElementType::Transaction => "TRANSACTION",
            ElementType::Descriptive => "DESCRIPTIVE",
            ElementType::Normative => "NORMATIVE",
            ElementType::Evaluative => "EVALUATIVE",
        }
    }

    pub fn from_db(value: &str) -> Option<ElementType> {
        match value {
            "TRANSACTION" | "transaction" => Some(ElementType::Transaction),
            "DESCRIPTIVE" | "descriptive" => Some(ElementType::Descriptive),
            "NORMATIVE" | "normative" => Some(ElementType::Normative),
            "EVALUATIVE" | "evaluative" => Some(ElementType::Evaluative),
            _ => ElementType::from_code(value),
        }
    }
}

impl From<ResourceType> for ElementType {
    fn from(resource_type: ResourceType) -> Self {
        match resource_type {
            ResourceType::Event => ElementType::Transaction,
            ResourceType::GeneralComment => ElementType::Descriptive,
            ResourceType::Element => ElementType::Normative,
            ResourceType::Feeling => ElementType::Evaluative,
            _ => ElementType::Transaction,
        }
    }
}

impl From<ElementType> for ResourceType {
    fn from(element_type: ElementType) -> Self {
        match element_type {
            ElementType::Transaction => ResourceType::Event,
            ElementType::Descriptive => ResourceType::GeneralComment,
            ElementType::Normative => ResourceType::Element,
            ElementType::Evaluative => ResourceType::Feeling,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementSubtype {
    Input,
    Output,
    Transformation,
    TransactionQuestion,
    Unit,
    DescriptiveQuestion,
    Theme,
    Plan,
    Obligation,
    Recommendation,
    Principle,
    Emotion,
    Energy,
    Quality,
    Interest,
}

impl ElementSubtype {
    pub fn to_code(self) -> &'static str {
        match self {
            ElementSubtype::Input => "input",
            ElementSubtype::Output => "output",
            ElementSubtype::Transformation => "transformation",
            ElementSubtype::TransactionQuestion => "transaction_question",
            ElementSubtype::Unit => "unit",
            ElementSubtype::DescriptiveQuestion => "descriptive_question",
            ElementSubtype::Theme => "theme",
            ElementSubtype::Plan => "plan",
            ElementSubtype::Obligation => "obligation",
            ElementSubtype::Recommendation => "recommendation",
            ElementSubtype::Principle => "principle",
            ElementSubtype::Emotion => "emotion",
            ElementSubtype::Energy => "energy",
            ElementSubtype::Quality => "quality",
            ElementSubtype::Interest => "interest",
        }
    }

    pub fn from_code(code: &str) -> Option<ElementSubtype> {
        match code {
            "input" => Some(ElementSubtype::Input),
            "output" => Some(ElementSubtype::Output),
            "transformation" => Some(ElementSubtype::Transformation),
            "transaction_question" => Some(ElementSubtype::TransactionQuestion),
            "unit" => Some(ElementSubtype::Unit),
            "descriptive_question" => Some(ElementSubtype::DescriptiveQuestion),
            "theme" => Some(ElementSubtype::Theme),
            "plan" => Some(ElementSubtype::Plan),
            "obligation" => Some(ElementSubtype::Obligation),
            "recommendation" => Some(ElementSubtype::Recommendation),
            "principle" => Some(ElementSubtype::Principle),
            "emotion" => Some(ElementSubtype::Emotion),
            "energy" => Some(ElementSubtype::Energy),
            "quality" => Some(ElementSubtype::Quality),
            "interest" => Some(ElementSubtype::Interest),
            _ => None,
        }
    }

    pub fn to_db(self) -> &'static str {
        match self {
            ElementSubtype::Input => "INPUT",
            ElementSubtype::Output => "OUTPUT",
            ElementSubtype::Transformation => "TRANSFORMATION",
            ElementSubtype::TransactionQuestion => "TRANSACTION_QUESTION",
            ElementSubtype::Unit => "UNIT",
            ElementSubtype::DescriptiveQuestion => "DESCRIPTIVE_QUESTION",
            ElementSubtype::Theme => "THEME",
            ElementSubtype::Plan => "PLAN",
            ElementSubtype::Obligation => "OBLIGATION",
            ElementSubtype::Recommendation => "RECOMMENDATION",
            ElementSubtype::Principle => "PRINCIPLE",
            ElementSubtype::Emotion => "EMOTION",
            ElementSubtype::Energy => "ENERGY",
            ElementSubtype::Quality => "QUALITY",
            ElementSubtype::Interest => "INTEREST",
        }
    }

    pub fn from_db(value: &str) -> Option<ElementSubtype> {
        match value {
            "INPUT" | "input" => Some(ElementSubtype::Input),
            "OUTPUT" | "output" => Some(ElementSubtype::Output),
            "TRANSFORMATION" | "transformation" => Some(ElementSubtype::Transformation),
            "TRANSACTION_QUESTION" | "transaction_question" => {
                Some(ElementSubtype::TransactionQuestion)
            }
            "UNIT" | "unit" => Some(ElementSubtype::Unit),
            "DESCRIPTIVE_QUESTION" | "descriptive_question" => {
                Some(ElementSubtype::DescriptiveQuestion)
            }
            "THEME" | "theme" => Some(ElementSubtype::Theme),
            "PLAN" | "plan" => Some(ElementSubtype::Plan),
            "OBLIGATION" | "obligation" => Some(ElementSubtype::Obligation),
            "RECOMMENDATION" | "recommendation" => Some(ElementSubtype::Recommendation),
            "PRINCIPLE" | "principle" => Some(ElementSubtype::Principle),
            "EMOTION" | "emotion" => Some(ElementSubtype::Emotion),
            "ENERGY" | "energy" => Some(ElementSubtype::Energy),
            "QUALITY" | "quality" => Some(ElementSubtype::Quality),
            "INTEREST" | "interest" => Some(ElementSubtype::Interest),
            _ => ElementSubtype::from_code(value),
        }
    }

    pub fn element_type(self) -> ElementType {
        match self {
            ElementSubtype::Input
            | ElementSubtype::Output
            | ElementSubtype::Transformation
            | ElementSubtype::TransactionQuestion => ElementType::Transaction,
            ElementSubtype::Unit
            | ElementSubtype::DescriptiveQuestion
            | ElementSubtype::Theme => ElementType::Descriptive,
            ElementSubtype::Plan
            | ElementSubtype::Obligation
            | ElementSubtype::Recommendation
            | ElementSubtype::Principle => ElementType::Normative,
            ElementSubtype::Emotion
            | ElementSubtype::Energy
            | ElementSubtype::Quality
            | ElementSubtype::Interest => ElementType::Evaluative,
        }
    }

    pub fn default_for_type(element_type: ElementType) -> ElementSubtype {
        match element_type {
            ElementType::Transaction => ElementSubtype::Output,
            ElementType::Descriptive => ElementSubtype::Unit,
            ElementType::Normative => ElementSubtype::Plan,
            ElementType::Evaluative => ElementSubtype::Emotion,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewElement {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub element_type: ElementType,
    pub element_subtype: ElementSubtype,
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
