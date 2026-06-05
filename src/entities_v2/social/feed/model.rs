use chrono::NaiveDateTime;
use serde::Serialize;
use uuid::Uuid;

use crate::entities_v2::post::PostStatus;

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeedSourceKind {
    Trace,
    Document,
    Album,
}

#[derive(Serialize, Debug, Clone)]
pub struct FeedItem {
    pub post_id: Uuid,
    pub source_kind: FeedSourceKind,
    pub source_id: Uuid,
    pub owner_user_id: Uuid,
    pub journal_id: Option<Uuid>,
    pub status: PostStatus,
    pub publishing_date: Option<NaiveDateTime>,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub cover_image_asset_id: Option<Uuid>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
