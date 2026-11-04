use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum NudgeType {
    ResumeDraft,
    WritingMemory,
    FirstTrace,
}

impl NudgeType {
    pub const ALL: [Self; 3] = [Self::ResumeDraft, Self::WritingMemory, Self::FirstTrace];

    pub fn to_db(self) -> &'static str {
        match self {
            Self::ResumeDraft => "RESUME_DRAFT",
            Self::WritingMemory => "WRITING_MEMORY",
            Self::FirstTrace => "FIRST_TRACE",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "RESUME_DRAFT" => Some(Self::ResumeDraft),
            "WRITING_MEMORY" => Some(Self::WritingMemory),
            "FIRST_TRACE" => Some(Self::FirstTrace),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NudgeEvent {
    Shown,
    Dismissed,
    Opened,
}

impl NudgeEvent {
    pub fn to_db(self) -> &'static str {
        match self {
            Self::Shown => "SHOWN",
            Self::Dismissed => "DISMISSED",
            Self::Opened => "OPENED",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "SHOWN" => Some(Self::Shown),
            "DISMISSED" => Some(Self::Dismissed),
            "OPENED" => Some(Self::Opened),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventInput {
    pub event: NudgeEvent,
}

#[derive(Serialize)]
pub struct CurrentNudge {
    pub nudge: Option<WritingNudge>,
}

#[derive(Serialize)]
pub struct WritingNudge {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub(super) kind: NudgeType,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub trace: Option<TracePreview>,
}

#[derive(Serialize)]
pub struct TracePreview {
    pub id: Uuid,
    pub journal_id: Option<Uuid>,
    pub title: String,
    pub excerpt: String,
    pub finalized_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}
