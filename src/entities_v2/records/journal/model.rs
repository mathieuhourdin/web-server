use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Journal {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
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
    #[serde(default)]
    pub journal_type: Option<JournalType>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct UpdateJournalDto {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<String>,
    pub journal_type: Option<JournalType>,
    pub status: Option<JournalStatus>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalType {
    MetaJournal,
    WorkLogJournal,
    ReadingNoteJournal,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalStatus {
    Draft,
    Published,
    Archived,
}

impl JournalType {
    pub fn to_db(self) -> &'static str {
        match self {
            JournalType::MetaJournal => "META_JOURNAL",
            JournalType::WorkLogJournal => "WORK_LOG_JOURNAL",
            JournalType::ReadingNoteJournal => "READING_NOTE_JOURNAL",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "META_JOURNAL" => JournalType::MetaJournal,
            "READING_NOTE_JOURNAL" => JournalType::ReadingNoteJournal,
            _ => JournalType::WorkLogJournal,
        }
    }
}

impl JournalStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            JournalStatus::Draft => "DRAFT",
            JournalStatus::Published => "PUBLISHED",
            JournalStatus::Archived => "ARCHIVED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "PUBLISHED" => JournalStatus::Published,
            "ARCHIVED" => JournalStatus::Archived,
            _ => JournalStatus::Draft,
        }
    }
}
