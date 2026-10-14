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
    pub post_id: Option<Uuid>,
    pub source_kind: FeedSourceKind,
    pub source_id: Uuid,
    pub continues_trace_id: Option<Uuid>,
    pub continues_from: Option<crate::entities_v2::trace::continuation::TraceContinuationPreview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_source_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_author_user_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_author_display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_author_profile_picture_display_url: Option<String>,
    pub owner_user_id: Uuid,
    pub journal_id: Option<Uuid>,
    pub status: PostStatus,
    pub publishing_date: Option<NaiveDateTime>,
    pub feed_date: NaiveDateTime,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub cover_image_asset_id: Option<Uuid>,
    pub cover_image: Option<crate::entities_v2::trace::TraceContentImage>,
    pub seen_by_preview: Option<crate::entities_v2::user_post_state::PostSeenByPreview>,
    pub owner_display_name: String,
    pub journal_title: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub visible_trace_date_range: Option<crate::entities_v2::album::AlbumTraceDateRange>,
}
