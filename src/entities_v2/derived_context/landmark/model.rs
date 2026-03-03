use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::shared::MaturingState;
use crate::entities_v2::element::model::Element;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Landmark {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub external_content_url: Option<String>,
    pub comment: Option<String>,
    pub image_url: Option<String>,
    pub landmark_type: LandmarkType,
    pub maturing_state: MaturingState,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

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
            "rsrc" | "RESOURCE" => Ok(LandmarkType::Resource),
            "them" | "TOPIC" => Ok(LandmarkType::Topic),
            "autr" | "PERSON" => Ok(LandmarkType::Person),
            "PROJECT" | "miss" => Ok(LandmarkType::Project),
            "HIGH_LEVEL_PROJECT" | "hlpr" => Ok(LandmarkType::HighLevelProject),
            "DELIVERABLE" | "dlvr" => Ok(LandmarkType::Deliverable),
            "ORGANIZATION" => Ok(LandmarkType::Organization),
            "TOOL" => Ok(LandmarkType::Tool),
            "QUESTION" | "qest" => Ok(LandmarkType::Question),
            "HABIT" => Ok(LandmarkType::Habit),
            "ROLE" => Ok(LandmarkType::Role),
            "SKILL" => Ok(LandmarkType::Skill),
            "PLACE" => Ok(LandmarkType::Place),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Invalid landmark type".to_string(),
            )),
        }
    }
}

impl Serialize for LandmarkType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_code())
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LandmarkWithParentsAndElements {
    #[serde(flatten)]
    pub landmark: Landmark,
    pub parent_landmarks: Vec<Landmark>,
    pub related_elements: Vec<Element>,
}

impl LandmarkWithParentsAndElements {
    pub fn new(
        landmark: Landmark,
        parent_landmarks: Vec<Landmark>,
        related_elements: Vec<Element>,
    ) -> Self {
        Self {
            landmark,
            parent_landmarks,
            related_elements,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewLandmark {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub landmark_type: LandmarkType,
    pub maturing_state: MaturingState,
    pub analysis_id: Uuid,
    pub user_id: Uuid,
    pub parent_id: Option<Uuid>,
}

impl NewLandmark {
    pub fn new(
        title: String,
        subtitle: String,
        content: String,
        landmark_type: LandmarkType,
        maturing_state: MaturingState,
        analysis_id: Uuid,
        user_id: Uuid,
        parent_id: Option<Uuid>,
    ) -> NewLandmark {
        Self {
            title,
            subtitle,
            content,
            landmark_type,
            maturing_state,
            analysis_id,
            user_id,
            parent_id,
        }
    }
}
