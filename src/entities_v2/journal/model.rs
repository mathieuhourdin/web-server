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
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub journal_type: JournalType,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalType {
    MetaJournal,
    WorkLogJournal,
    ReadingNoteJournal,
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
