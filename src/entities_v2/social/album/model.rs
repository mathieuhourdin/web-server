use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities_v2::post::PostAudienceRole;

pub use super::enums::{AlbumCompletionStatus, AlbumOrderingMode, AlbumVisibility};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Album {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub ordering_mode: AlbumOrderingMode,
    pub completion_status: AlbumCompletionStatus,
    pub visibility: AlbumVisibility,
    pub cover_image_asset_id: Option<Uuid>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AlbumItem {
    pub id: Uuid,
    pub album_id: Uuid,
    pub trace_id: Uuid,
    pub ordering_index: i32,
    pub created_at: NaiveDateTime,
}

#[derive(Serialize, Debug, Clone)]
pub struct AlbumItemTraceView {
    pub id: Uuid,
    pub journal_id: Option<Uuid>,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub content_image_asset_id: Option<Uuid>,
    pub interaction_date: NaiveDateTime,
    pub published_post_id: Option<Uuid>,
    pub publishing_date: Option<NaiveDateTime>,
    pub post_audience_role: Option<PostAudienceRole>,
}

#[derive(Serialize, Debug, Clone)]
pub struct AlbumItemView {
    pub id: Uuid,
    pub album_id: Uuid,
    pub trace_id: Uuid,
    pub ordering_index: i32,
    pub created_at: NaiveDateTime,
    pub trace: AlbumItemTraceView,
}

#[derive(Serialize, Debug, Clone)]
pub struct AlbumWithItems {
    #[serde(flatten)]
    pub album: Album,
    pub items: Vec<AlbumItemView>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NewAlbumDto {
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub ordering_mode: Option<AlbumOrderingMode>,
    #[serde(default)]
    pub completion_status: Option<AlbumCompletionStatus>,
    #[serde(default)]
    pub visibility: Option<AlbumVisibility>,
    #[serde(default)]
    #[serde(alias = "cover_asset_id")]
    pub cover_image_asset_id: Option<Uuid>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct UpdateAlbumDto {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<String>,
    pub ordering_mode: Option<AlbumOrderingMode>,
    pub completion_status: Option<AlbumCompletionStatus>,
    pub visibility: Option<AlbumVisibility>,
    #[serde(alias = "cover_asset_id")]
    pub cover_image_asset_id: Option<Option<Uuid>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NewAlbumItemDto {
    pub trace_id: Uuid,
    pub ordering_index: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct NewAlbum {
    pub owner_user_id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub ordering_mode: AlbumOrderingMode,
    pub completion_status: AlbumCompletionStatus,
    pub visibility: AlbumVisibility,
    pub cover_image_asset_id: Option<Uuid>,
}

impl NewAlbum {
    pub fn new(payload: NewAlbumDto, owner_user_id: Uuid) -> Self {
        Self {
            owner_user_id,
            title: payload.title,
            subtitle: payload.subtitle.unwrap_or_default(),
            content: payload.content.unwrap_or_default(),
            ordering_mode: payload
                .ordering_mode
                .unwrap_or(AlbumOrderingMode::Chronological),
            completion_status: payload
                .completion_status
                .unwrap_or(AlbumCompletionStatus::InProgress),
            visibility: payload.visibility.unwrap_or(AlbumVisibility::Private),
            cover_image_asset_id: payload.cover_image_asset_id,
        }
    }
}
