use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities_v2::user::UserPublicResponse;

pub use super::enums::{JournalExportFormat, JournalSharingMode, JournalStatus, JournalType};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Journal {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub is_encrypted: bool,
    pub last_trace_at: Option<NaiveDateTime>,
    pub current_draft_id: Option<Uuid>,
    pub status: JournalStatus,
    pub journal_type: JournalType,
    pub sharing_mode: JournalSharingMode,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// A named account that either has a future-sharing default for the journal or
/// can currently read at least one of its traces. The owner is not included.
#[derive(Serialize)]
pub struct JournalAudienceMember {
    pub user: UserPublicResponse,
    pub active_default_sharing_policy: Option<JournalAudienceDefaultSharingPolicy>,
    pub accessible_trace_count: i64,
    pub access_via: Vec<JournalAudienceAccessVia>,
}

#[derive(Serialize)]
pub struct JournalAudienceDefaultSharingPolicy {
    pub id: Uuid,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalAudienceAccessVia {
    DirectPostGrant,
    Mention,
    Reshare,
}

impl JournalAudienceAccessVia {
    pub(super) fn from_query_value(value: &str) -> Option<Self> {
        match value {
            "direct_post_grant" => Some(Self::DirectPostGrant),
            "mention" => Some(Self::Mention),
            "reshare" => Some(Self::Reshare),
            _ => None,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct NewJournalDto {
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default, alias = "is_ecrypted")]
    pub is_encrypted: Option<bool>,
    #[serde(default)]
    pub journal_type: Option<JournalType>,
    #[serde(default)]
    pub sharing_mode: Option<JournalSharingMode>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct UpdateJournalDto {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<String>,
    #[serde(default, alias = "is_ecrypted")]
    pub is_encrypted: Option<bool>,
    pub journal_type: Option<JournalType>,
    pub sharing_mode: Option<JournalSharingMode>,
    pub status: Option<JournalStatus>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct JournalExportDto {
    pub format: JournalExportFormat,
    #[serde(default)]
    pub include_messages: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct JournalExportResponse {
    pub format: JournalExportFormat,
    pub content: String,
}
