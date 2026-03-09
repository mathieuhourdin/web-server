use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities_v2::element::model::Element;
use crate::entities_v2::shared::MaturingState;

pub use super::enums::{LandmarkType, LandmarkTypeLegacy};

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
