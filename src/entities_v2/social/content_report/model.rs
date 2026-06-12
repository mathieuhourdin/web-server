use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub use super::enums::{ContentReportReason, ContentReportSourceKind, ContentReportStatus};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContentReport {
    pub id: Uuid,
    pub reporter_user_id: Uuid,
    pub reported_message_id: Option<Uuid>,
    pub reported_post_id: Option<Uuid>,
    pub reported_user_id: Uuid,
    pub reason: ContentReportReason,
    pub reporter_comment: String,
    pub status: ContentReportStatus,
    pub reviewed_by_user_id: Option<Uuid>,
    pub reviewed_at: Option<NaiveDateTime>,
    pub resolution_note: String,
    pub snapshot_title: String,
    pub snapshot_content: String,
    pub snapshot_attachment_json: Option<Value>,
    pub snapshot_metadata_json: Option<Value>,
    pub snapshot_source_kind: Option<ContentReportSourceKind>,
    pub snapshot_source_id: Option<Uuid>,
    pub snapshot_context_json: Option<Value>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NewContentReportDto {
    #[serde(default)]
    pub reported_message_id: Option<Uuid>,
    #[serde(default)]
    pub reported_post_id: Option<Uuid>,
    pub reason: ContentReportReason,
    #[serde(default)]
    pub reporter_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewContentReport {
    pub reporter_user_id: Uuid,
    pub reported_message_id: Option<Uuid>,
    pub reported_post_id: Option<Uuid>,
    pub reported_user_id: Uuid,
    pub reason: ContentReportReason,
    pub reporter_comment: String,
    pub snapshot_title: String,
    pub snapshot_content: String,
    pub snapshot_attachment_json: Option<Value>,
    pub snapshot_metadata_json: Option<Value>,
    pub snapshot_source_kind: Option<ContentReportSourceKind>,
    pub snapshot_source_id: Option<Uuid>,
    pub snapshot_context_json: Option<Value>,
}
