use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    interaction::model::NewInteraction,
    resource::{maturing_state::MaturingState, Resource},
    resource_relation::NewResourceRelation,
};
use crate::entities_v2::element::model::Element;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub publishing_state: String,
    pub category_id: Option<Uuid>,
    pub is_external: bool,
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
            "rsrc" => Ok(LandmarkType::Resource),
            "them" => Ok(LandmarkType::Topic),
            "autr" => Ok(LandmarkType::Person),
            "RESOURCE" => Ok(LandmarkType::Resource),
            "PROJECT" => Ok(LandmarkType::Project),
            "DELIVERABLE" => Ok(LandmarkType::Deliverable),
            "PERSON" => Ok(LandmarkType::Person),
            "ORGANIZATION" => Ok(LandmarkType::Organization),
            "TOPIC" => Ok(LandmarkType::Topic),
            "TOOL" => Ok(LandmarkType::Tool),
            "QUESTION" => Ok(LandmarkType::Question),
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
    pub publishing_state: String,
    pub analysis_id: Uuid,
    pub user_id: Uuid,
    pub parent_id: Option<Uuid>,
}

impl Landmark {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Landmark, PpdcError> {
        let result = Resource::find(id, pool)?;
        Ok(Landmark::from_resource(result))
    }
    pub fn update(self, pool: &DbPool) -> Result<Landmark, PpdcError> {
        let result = self.to_resource();
        let updated_resource = result.update(pool)?;
        Ok(Landmark::from_resource(updated_resource))
    }
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
            publishing_state: "pbsh".to_string(),
            analysis_id,
            user_id,
            parent_id,
        }
    }

    pub fn create(self, pool: &DbPool) -> Result<Landmark, PpdcError> {
        let analysis_id = self.analysis_id;
        let user_id = self.user_id;
        let parent_id = self.parent_id;

        // create the landmark with all positive flags
        let landmark = self;
        let new_resource = landmark.to_new_resource();
        let created_resource = new_resource.create(pool)?;
        let landmark = Landmark::from_resource(created_resource);

        // Create relation with analysis
        let mut new_resource_relation = NewResourceRelation::new(landmark.id, analysis_id);
        new_resource_relation.relation_type = Some("ownr".to_string());
        new_resource_relation.user_id = Some(user_id);
        new_resource_relation.create(pool)?;

        if let Some(parent_id) = parent_id {
            // create the parent relation to the parent landmark
            let mut new_resource_relation = NewResourceRelation::new(landmark.id, parent_id);
            new_resource_relation.relation_type = Some("prnt".to_string());
            new_resource_relation.user_id = Some(user_id);
            new_resource_relation.create(pool)?;
        }

        // link the landmark to the user with the interaction
        let mut new_interaction = NewInteraction::new(user_id, landmark.id);
        new_interaction.interaction_type = Some("anly".to_string());
        new_interaction.create(pool)?;
        Ok(landmark)
    }
}
