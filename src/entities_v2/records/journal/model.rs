use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::enums::{JournalExportFormat, JournalStatus, JournalType};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Journal {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub is_encrypted: bool,
    pub last_trace_at: Option<NaiveDateTime>,
    pub user_id: Uuid,
    pub status: JournalStatus,
    pub journal_type: JournalType,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
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
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct UpdateJournalDto {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<String>,
    #[serde(default, alias = "is_ecrypted")]
    pub is_encrypted: Option<bool>,
    pub journal_type: Option<JournalType>,
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
