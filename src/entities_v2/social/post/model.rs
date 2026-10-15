use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::enums::{PostInteractionType, PostStatus, PostType};

pub(crate) fn reject_removed_audience_role<'de, D>(deserializer: D) -> Result<(), D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde::de::IgnoredAny::deserialize(deserializer)?;
    Err(serde::de::Error::custom(
        "audience_role has been removed; configure grants before publishing",
    ))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Post {
    pub id: Uuid,
    #[serde(skip_serializing, skip_deserializing)]
    pub resource_id: Uuid,
    pub source_trace_id: Option<Uuid>,
    pub source_document_id: Option<Uuid>,
    pub source_album_id: Option<Uuid>,
    pub interaction_type: PostInteractionType,
    pub post_type: PostType,
    pub user_id: Uuid,
    pub publishing_date: Option<NaiveDateTime>,
    pub status: PostStatus,
    pub notification_mode: super::notifications::PublicationNotificationMode,
    // Recipient selection is private to the publisher; use the owner-only settings route.
    #[serde(skip)]
    pub notification_user_ids: Vec<Uuid>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FeedPostResponse {
    #[serde(flatten)]
    pub post: Post,
    pub journal_id: Option<Uuid>,
    pub journal_title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostSourceRef {
    Trace(Uuid),
    Document(Uuid),
    Album(Uuid),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewPostDto {
    #[serde(flatten)]
    pub notifications: super::notifications::NotificationOptions,
    #[serde(
        rename = "audience_role",
        default,
        deserialize_with = "reject_removed_audience_role",
        skip_serializing
    )]
    pub removed_audience_role: (),
    #[serde(default)]
    pub source_trace_id: Option<Uuid>,
    #[serde(default)]
    pub source_document_id: Option<Uuid>,
    #[serde(default)]
    pub source_album_id: Option<Uuid>,
    pub post_type: Option<PostType>,
    pub interaction_type: Option<PostInteractionType>,
    pub publishing_date: Option<NaiveDateTime>,
    pub status: Option<PostStatus>,
}

#[derive(Debug, Clone)]
pub struct NewPost {
    pub source_trace_id: Option<Uuid>,
    pub source_document_id: Option<Uuid>,
    pub source_album_id: Option<Uuid>,
    pub post_type: PostType,
    pub interaction_type: PostInteractionType,
    pub user_id: Uuid,
    pub publishing_date: Option<NaiveDateTime>,
    pub status: PostStatus,
}

impl NewPost {
    pub fn new(payload: NewPostDto, user_id: Uuid) -> Self {
        let status = payload.status.unwrap_or(PostStatus::Draft);
        Self {
            source_trace_id: payload.source_trace_id,
            source_document_id: payload.source_document_id,
            source_album_id: payload.source_album_id,
            post_type: payload.post_type.unwrap_or(PostType::Idea),
            interaction_type: payload
                .interaction_type
                .unwrap_or(PostInteractionType::Output),
            user_id,
            publishing_date: payload.publishing_date,
            status,
        }
    }
}

impl Post {
    pub fn source_ref(&self) -> Option<PostSourceRef> {
        if let Some(trace_id) = self.source_trace_id {
            Some(PostSourceRef::Trace(trace_id))
        } else if let Some(document_id) = self.source_document_id {
            Some(PostSourceRef::Document(document_id))
        } else {
            self.source_album_id.map(PostSourceRef::Album)
        }
    }
}
