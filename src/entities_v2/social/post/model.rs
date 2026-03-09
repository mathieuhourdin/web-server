use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities_v2::shared::MaturingState;

pub use super::enums::{PostInteractionType, PostType};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Post {
    pub id: Uuid,
    #[serde(skip_serializing, skip_deserializing)]
    pub resource_id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub image_url: Option<String>,
    pub interaction_type: PostInteractionType,
    pub post_type: PostType,
    pub user_id: Uuid,
    pub publishing_date: Option<NaiveDateTime>,
    pub publishing_state: String,
    pub maturing_state: MaturingState,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewPostDto {
    pub title: String,
    pub subtitle: Option<String>,
    pub content: String,
    pub image_url: Option<String>,
    pub post_type: Option<PostType>,
    pub interaction_type: Option<PostInteractionType>,
    pub publishing_date: Option<NaiveDateTime>,
    pub publishing_state: Option<String>,
    pub maturing_state: Option<MaturingState>,
}

#[derive(Debug, Clone)]
pub struct NewPost {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub image_url: Option<String>,
    pub post_type: PostType,
    pub interaction_type: PostInteractionType,
    pub user_id: Uuid,
    pub publishing_date: Option<NaiveDateTime>,
    pub publishing_state: String,
    pub maturing_state: MaturingState,
}

impl NewPost {
    pub fn new(payload: NewPostDto, user_id: Uuid) -> Self {
        Self {
            title: payload.title,
            subtitle: payload.subtitle.unwrap_or_default(),
            content: payload.content,
            image_url: payload.image_url,
            post_type: payload.post_type.unwrap_or(PostType::Idea),
            interaction_type: payload
                .interaction_type
                .unwrap_or(PostInteractionType::Output),
            user_id,
            publishing_date: payload.publishing_date,
            publishing_state: payload
                .publishing_state
                .unwrap_or_else(|| "pbsh".to_string()),
            maturing_state: payload.maturing_state.unwrap_or(MaturingState::Draft),
        }
    }
}
