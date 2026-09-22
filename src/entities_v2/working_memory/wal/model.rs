use chrono::{NaiveDate, NaiveDateTime};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::{wal_days, wal_entries, wal_projections};

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = wal_days)]
pub struct WalDay {
    pub id: Uuid,
    pub user_id: Uuid,
    pub local_date: NaiveDate,
    pub input: String,
    pub context: String,
    pub compiled_operational: Option<String>,
    pub compiled_thematic: Option<String>,
    pub compiled_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub input_revision: i64,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = wal_days)]
pub(crate) struct NewWalDay {
    pub id: Uuid,
    pub user_id: Uuid,
    pub local_date: NaiveDate,
    pub input: String,
    pub context: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Identifiable, Associations)]
#[diesel(table_name = wal_entries)]
#[diesel(belongs_to(WalDay))]
pub struct WalEntry {
    pub id: Uuid,
    pub wal_day_id: Uuid,
    pub position: i32,
    pub content: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = wal_entries)]
pub(crate) struct NewWalEntry {
    pub id: Uuid,
    pub wal_day_id: Uuid,
    pub position: i32,
    pub content: String,
}

#[derive(Debug, Clone, Queryable, Selectable, Identifiable, Associations)]
#[diesel(table_name = wal_projections)]
#[diesel(belongs_to(WalDay))]
pub struct WalProjection {
    pub id: Uuid,
    pub wal_day_id: Uuid,
    pub projection_type: String,
    pub target_date: Option<NaiveDate>,
    pub status: String,
    pub source_revision: i64,
    pub schema_version: i32,
    pub prompt_version: String,
    pub content: Option<Value>,
    pub error_message: Option<String>,
    pub generated_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalProjectionType {
    Operational,
    Thematic,
    Carryover,
}

impl WalProjectionType {
    pub fn to_db(self) -> &'static str {
        match self {
            Self::Operational => "OPERATIONAL",
            Self::Thematic => "THEMATIC",
            Self::Carryover => "CARRYOVER",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, PpdcError> {
        match value {
            "OPERATIONAL" => Ok(Self::Operational),
            "THEMATIC" => Ok(Self::Thematic),
            "CARRYOVER" => Ok(Self::Carryover),
            _ => Err(PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unknown WAL projection type: {value}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WalProjectionStatus {
    Pending,
    Processing,
    Ready,
    Failed,
    SkippedAiDisabled,
    Stale,
}

impl WalProjectionStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Processing => "PROCESSING",
            Self::Ready => "READY",
            Self::Failed => "FAILED",
            Self::SkippedAiDisabled => "SKIPPED_AI_DISABLED",
            Self::Stale => "STALE",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, PpdcError> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "PROCESSING" => Ok(Self::Processing),
            "READY" => Ok(Self::Ready),
            "FAILED" => Ok(Self::Failed),
            "SKIPPED_AI_DISABLED" => Ok(Self::SkippedAiDisabled),
            "STALE" => Ok(Self::Stale),
            _ => Err(PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unknown WAL projection status: {value}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WalProjectionItemStatus {
    Open,
    Done,
    Mixed,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalProjectionItem {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub status: WalProjectionItemStatus,
    pub source_entry_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalProjectionSection {
    pub key: String,
    pub label: String,
    pub items: Vec<WalProjectionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalStructuredProjection {
    pub schema_version: i32,
    pub sections: Vec<WalProjectionSection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalProjectionView {
    pub id: Uuid,
    pub status: WalProjectionStatus,
    pub source_revision: i64,
    pub schema_version: i32,
    pub prompt_version: String,
    pub content: WalStructuredProjection,
    pub generated_at: Option<NaiveDateTime>,
    pub is_stale: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalCompilation {
    pub content: String,
    pub compiled_at: NaiveDateTime,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct WalCompilationViews {
    pub operational: Option<WalCompilation>,
    pub thematic: Option<WalCompilation>,
    pub operational_projection: Option<WalProjectionView>,
    pub thematic_projection: Option<WalProjectionView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalEntryResponse {
    pub id: Uuid,
    pub position: i32,
    pub content: String,
    pub created_at: NaiveDateTime,
}

impl From<WalEntry> for WalEntryResponse {
    fn from(entry: WalEntry) -> Self {
        Self {
            id: entry.id,
            position: entry.position,
            content: entry.content,
            created_at: entry.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct WalResponse {
    pub id: Uuid,
    pub date: NaiveDate,
    pub content: String,
    pub entries: Vec<WalEntryResponse>,
}

impl WalResponse {
    pub fn new(wal: &WalDay, entries: Vec<WalEntry>) -> Self {
        Self {
            id: wal.id,
            date: wal.local_date,
            content: wal.input.clone(),
            entries: entries.into_iter().map(WalEntryResponse::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct WalDayResponse {
    pub id: Uuid,
    pub date: NaiveDate,
    pub content: String,
    pub operational: Option<WalCompilation>,
    pub thematic: Option<WalCompilation>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl WalDay {
    pub fn legacy_compilation_views(&self) -> WalCompilationViews {
        WalCompilationViews {
            operational: self
                .compiled_operational
                .as_ref()
                .zip(self.compiled_at)
                .map(|(content, compiled_at)| WalCompilation {
                    content: content.clone(),
                    compiled_at,
                }),
            thematic: self.compiled_thematic.as_ref().zip(self.compiled_at).map(
                |(content, compiled_at)| WalCompilation {
                    content: content.clone(),
                    compiled_at,
                },
            ),
            operational_projection: None,
            thematic_projection: None,
        }
    }
}

impl From<WalDay> for WalDayResponse {
    fn from(wal: WalDay) -> Self {
        let views = wal.legacy_compilation_views();
        Self {
            id: wal.id,
            date: wal.local_date,
            content: wal.input,
            operational: views.operational,
            thematic: views.thematic,
            created_at: wal.created_at,
            updated_at: wal.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WalCarryoverAssessment {
    ExplicitForToday,
    LikelyOpen,
    Uncertain,
    ProbablyClosed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WalCarryoverApplicationStatus {
    Pending,
    Accepted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalCarryoverApplication {
    pub status: WalCarryoverApplicationStatus,
    pub target_entry_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalCarryoverItem {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub assessment: WalCarryoverAssessment,
    pub reason: String,
    pub selected_by_default: bool,
    pub source_entry_ids: Vec<Uuid>,
    pub application: WalCarryoverApplication,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalCarryoverContent {
    pub schema_version: i32,
    pub source_date: NaiveDate,
    pub target_date: NaiveDate,
    pub items: Vec<WalCarryoverItem>,
}

#[derive(Debug, Serialize)]
pub struct WalCarryoverResponse {
    pub projection_id: Option<Uuid>,
    pub source_date: NaiveDate,
    pub target_date: NaiveDate,
    pub status: WalCarryoverResponseStatus,
    pub items: Vec<WalCarryoverItem>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WalCarryoverResponseStatus {
    NotApplicable,
    Pending,
    Processing,
    Ready,
    Failed,
    SkippedAiDisabled,
    Stale,
}

impl From<WalProjectionStatus> for WalCarryoverResponseStatus {
    fn from(status: WalProjectionStatus) -> Self {
        match status {
            WalProjectionStatus::Pending => Self::Pending,
            WalProjectionStatus::Processing => Self::Processing,
            WalProjectionStatus::Ready => Self::Ready,
            WalProjectionStatus::Failed => Self::Failed,
            WalProjectionStatus::SkippedAiDisabled => Self::SkippedAiDisabled,
            WalProjectionStatus::Stale => Self::Stale,
        }
    }
}
