use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities_v2::shared::MaturingState;

pub use super::enums::{PostInteractionType, PostStatus, PostType};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Post {
    pub id: Uuid,
    #[serde(skip_serializing, skip_deserializing)]
    pub resource_id: Uuid,
    pub source_trace_id: Option<Uuid>,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub image_url: Option<String>,
    pub interaction_type: PostInteractionType,
    pub post_type: PostType,
    pub user_id: Uuid,
    pub publishing_date: Option<NaiveDateTime>,
    pub status: PostStatus,
    #[serde(skip_serializing, default = "default_post_publishing_state")]
    pub publishing_state: String,
    #[serde(skip_serializing, default = "default_post_maturing_state")]
    pub maturing_state: MaturingState,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewPostDto {
    #[serde(default)]
    pub source_trace_id: Option<Uuid>,
    pub title: String,
    pub subtitle: Option<String>,
    pub content: String,
    pub image_url: Option<String>,
    pub post_type: Option<PostType>,
    pub interaction_type: Option<PostInteractionType>,
    pub publishing_date: Option<NaiveDateTime>,
    pub status: Option<PostStatus>,
}

#[derive(Debug, Clone)]
pub struct NewPost {
    pub source_trace_id: Option<Uuid>,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub image_url: Option<String>,
    pub post_type: PostType,
    pub interaction_type: PostInteractionType,
    pub user_id: Uuid,
    pub publishing_date: Option<NaiveDateTime>,
    pub status: PostStatus,
    pub publishing_state: String,
    pub maturing_state: MaturingState,
}

fn default_post_publishing_state() -> String {
    "pbsh".to_string()
}

fn default_post_maturing_state() -> MaturingState {
    MaturingState::Draft
}

pub fn legacy_lifecycle_for_status(status: PostStatus) -> (String, MaturingState) {
    match status {
        PostStatus::Draft => ("pbsh".to_string(), MaturingState::Draft),
        PostStatus::Published => ("pbsh".to_string(), MaturingState::Finished),
        PostStatus::Archived => ("pbsh".to_string(), MaturingState::Trashed),
    }
}

impl NewPost {
    pub fn new(payload: NewPostDto, user_id: Uuid) -> Self {
        let status = payload.status.unwrap_or(PostStatus::Draft);
        let (publishing_state, maturing_state) = legacy_lifecycle_for_status(status);
        Self {
            source_trace_id: payload.source_trace_id,
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
            status,
            publishing_state,
            maturing_state,
        }
    }
}
