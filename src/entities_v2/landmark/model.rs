use uuid::Uuid;
use crate::entities::{
    error::{ErrorType, PpdcError},
    resource::{
        Resource,
        maturing_state::MaturingState,
    },
    resource_relation::NewResourceRelation,
    interaction::model::NewInteraction,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use crate::db::DbPool;

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
pub enum LandmarkType {
    Resource,
    Theme,
    Author,
}

impl LandmarkType {
    pub fn to_code(&self) -> &'static str {
        match self {
            LandmarkType::Resource => "rsrc",
            LandmarkType::Theme => "them",
            LandmarkType::Author => "autr",
        }
    }
    pub fn from_code(code: &str) -> Result<Self, PpdcError> {
        match code {
            "rsrc" => Ok(LandmarkType::Resource),
            "them" => Ok(LandmarkType::Theme),
            "autr" => Ok(LandmarkType::Author),
            _ => Err(PpdcError::new(400, ErrorType::ApiError, "Invalid landmark type".to_string())),
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
    pub related_elements: Vec<Resource>,
}

impl LandmarkWithParentsAndElements {
    pub fn new(landmark: Landmark, parent_landmarks: Vec<Landmark>, related_elements: Vec<Resource>) -> Self {
        Self { landmark, parent_landmarks, related_elements }
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
            parent_id
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
