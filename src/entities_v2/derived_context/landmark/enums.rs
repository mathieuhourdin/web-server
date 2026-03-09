use serde::{Deserialize, Serialize};

use crate::entities_v2::error::{ErrorType, PpdcError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LandmarkTypeLegacy {
    Resource,
    Theme,
    Author,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LandmarkType {
    Project,
    HighLevelProject,
    Deliverable,
    Resource,
    Person,
    Organization,
    Topic,
    Tool,
    Question,
    Habit,
    Role,
    Skill,
    Place,
}

impl LandmarkType {
    pub fn to_code(&self) -> &'static str {
        match self {
            LandmarkType::Resource => "RESOURCE",
            LandmarkType::Project => "PROJECT",
            LandmarkType::HighLevelProject => "HIGH_LEVEL_PROJECT",
            LandmarkType::Deliverable => "DELIVERABLE",
            LandmarkType::Person => "PERSON",
            LandmarkType::Organization => "ORGANIZATION",
            LandmarkType::Topic => "TOPIC",
            LandmarkType::Tool => "TOOL",
            LandmarkType::Question => "QUESTION",
            LandmarkType::Habit => "HABIT",
            LandmarkType::Role => "ROLE",
            LandmarkType::Skill => "SKILL",
            LandmarkType::Place => "PLACE",
        }
    }

    pub fn from_code(code: &str) -> Result<Self, PpdcError> {
        match code {
            "rsrc" | "RESOURCE" | "resource" => Ok(LandmarkType::Resource),
            "them" | "TOPIC" | "topic" => Ok(LandmarkType::Topic),
            "autr" | "PERSON" | "person" => Ok(LandmarkType::Person),
            "PROJECT" | "miss" | "project" => Ok(LandmarkType::Project),
            "HIGH_LEVEL_PROJECT" | "hlpr" | "high_level_project" => {
                Ok(LandmarkType::HighLevelProject)
            }
            "DELIVERABLE" | "dlvr" | "deliverable" => Ok(LandmarkType::Deliverable),
            "ORGANIZATION" | "organization" => Ok(LandmarkType::Organization),
            "TOOL" | "tool" => Ok(LandmarkType::Tool),
            "QUESTION" | "qest" | "question" => Ok(LandmarkType::Question),
            "HABIT" | "habit" => Ok(LandmarkType::Habit),
            "ROLE" | "role" => Ok(LandmarkType::Role),
            "SKILL" | "skill" => Ok(LandmarkType::Skill),
            "PLACE" | "place" => Ok(LandmarkType::Place),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Invalid landmark type".to_string(),
            )),
        }
    }

    pub fn to_api_value(&self) -> &'static str {
        match self {
            LandmarkType::Project => "project",
            LandmarkType::HighLevelProject => "high_level_project",
            LandmarkType::Deliverable => "deliverable",
            LandmarkType::Resource => "resource",
            LandmarkType::Person => "person",
            LandmarkType::Organization => "organization",
            LandmarkType::Topic => "topic",
            LandmarkType::Tool => "tool",
            LandmarkType::Question => "question",
            LandmarkType::Habit => "habit",
            LandmarkType::Role => "role",
            LandmarkType::Skill => "skill",
            LandmarkType::Place => "place",
        }
    }
}

impl Serialize for LandmarkType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_api_value())
    }
}

impl<'de> Deserialize<'de> for LandmarkType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(LandmarkType::from_code(&s).unwrap_or(LandmarkType::Resource))
    }
}
