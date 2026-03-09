use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalExportFormat {
    Markdown,
    Text,
    Json,
}

impl JournalExportFormat {
    pub fn to_api_value(self) -> &'static str {
        match self {
            JournalExportFormat::Markdown => "md",
            JournalExportFormat::Text => "txt",
            JournalExportFormat::Json => "json",
        }
    }

    pub fn from_api_value(value: &str) -> Option<Self> {
        let normalized = value.trim().trim_start_matches('.').to_ascii_lowercase();
        match normalized.as_str() {
            "md" | "markdown" => Some(JournalExportFormat::Markdown),
            "txt" | "text" => Some(JournalExportFormat::Text),
            "json" => Some(JournalExportFormat::Json),
            _ => None,
        }
    }
}

impl Serialize for JournalExportFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_api_value())
    }
}

impl<'de> Deserialize<'de> for JournalExportFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        JournalExportFormat::from_api_value(&raw)
            .ok_or_else(|| de::Error::custom("unknown format. expected one of: md, txt, json"))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalType {
    MetaJournal,
    WorkLogJournal,
    ReadingNoteJournal,
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalStatus {
    Draft,
    Published,
    Archived,
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
