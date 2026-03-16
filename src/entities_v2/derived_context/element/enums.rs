use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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
            ElementSubtype::Unit | ElementSubtype::DescriptiveQuestion | ElementSubtype::Theme => {
                ElementType::Descriptive
            }
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ElementStatus {
    Done,
    InProgress,
    Intended,
}

impl ElementStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            ElementStatus::Done => "DONE",
            ElementStatus::InProgress => "IN_PROGRESS",
            ElementStatus::Intended => "INTENDED",
        }
    }

    pub fn from_db(value: &str) -> Option<ElementStatus> {
        match value {
            "DONE" | "done" => Some(ElementStatus::Done),
            "IN_PROGRESS" | "in_progress" => Some(ElementStatus::InProgress),
            "INTENDED" | "intended" => Some(ElementStatus::Intended),
            _ => None,
        }
    }
}
