use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::resource::resource_type::ResourceType;

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

impl From<ResourceType> for JournalType {
    fn from(resource_type: ResourceType) -> Self {
        match resource_type {
            ResourceType::MetaJournal => JournalType::MetaJournal,
            ResourceType::ReadingNoteJournal => JournalType::ReadingNoteJournal,
            ResourceType::WorkLogJournal => JournalType::WorkLogJournal,
            // Backward compatibility for older journal resources.
            ResourceType::Journal => JournalType::WorkLogJournal,
            _ => JournalType::WorkLogJournal,
        }
    }
}

impl From<JournalType> for ResourceType {
    fn from(journal_type: JournalType) -> Self {
        match journal_type {
            JournalType::MetaJournal => ResourceType::MetaJournal,
            JournalType::WorkLogJournal => ResourceType::WorkLogJournal,
            JournalType::ReadingNoteJournal => ResourceType::ReadingNoteJournal,
        }
    }
}
