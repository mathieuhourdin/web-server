use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types::{Bool, Nullable, Text, Timestamp, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalHistoryDecision {
    None,
    AllNormal,
    AllIncludingSensitive,
    UserSelected,
}

impl JournalHistoryDecision {
    pub fn to_db(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::AllNormal => "ALL_NORMAL",
            Self::AllIncludingSensitive => "ALL_INCLUDING_SENSITIVE",
            Self::UserSelected => "USER_SELECTED",
        }
    }
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "NONE" => Some(Self::None),
            "ALL_NORMAL" | "NORMAL_ONLY" => Some(Self::AllNormal),
            "ALL_INCLUDING_SENSITIVE" | "ALL" => Some(Self::AllIncludingSensitive),
            "USER_SELECTED" | "SPECIFIC" => Some(Self::UserSelected),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudienceReviewReason {
    FutureDefault,
    History,
}
use crate::pagination::PaginationParams;

#[derive(Debug, Clone, Serialize, QueryableByName)]
pub struct Audience {
    #[diesel(sql_type = SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    pub owner_user_id: Uuid,
    #[diesel(sql_type = Text)]
    pub name: String,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    pub source_journal_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<Timestamp>)]
    pub archived_at: Option<NaiveDateTime>,
    #[diesel(sql_type = Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    pub updated_at: NaiveDateTime,
}

/// Read projection; trace_count is computed, never persisted on the audience.
#[derive(Debug, Serialize)]
pub struct AudienceView {
    #[serde(flatten)]
    pub audience: Audience,
    pub trace_count: i64,
}

#[derive(Debug, Clone, Serialize, QueryableByName)]
pub struct AudienceMember {
    #[diesel(sql_type = SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    pub audience_id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    pub user_id: Uuid,
    #[diesel(sql_type = Text)]
    pub status: String,
    #[diesel(sql_type = Bool)]
    pub future_access_enabled: bool,
    #[diesel(sql_type = Timestamp)]
    pub history_cutoff_at: NaiveDateTime,
    #[diesel(sql_type = Text)]
    pub history_review_state: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub history_decision: Option<String>,
    #[diesel(sql_type = Nullable<Timestamp>)]
    pub history_reviewed_at: Option<NaiveDateTime>,
    #[serde(skip_serializing)]
    #[diesel(sql_type = Nullable<SqlUuid>)]
    pub legacy_policy_id: Option<Uuid>,
    #[diesel(sql_type = Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    pub updated_at: NaiveDateTime,
}

impl AudienceMember {
    pub(crate) fn normalize(mut self) -> Self {
        self.status = self.status.to_lowercase();
        self.history_review_state = self.history_review_state.to_lowercase();
        self.history_decision = self.history_decision.map(|value| value.to_lowercase());
        self
    }
}

#[derive(Serialize)]
pub struct MemberView {
    #[serde(flatten)]
    pub member: AudienceMember,
    pub user: crate::entities_v2::user::UserPublicResponse,
}

#[derive(Serialize)]
pub struct PendingReview {
    pub audience: Audience,
    pub member: AudienceMember,
    pub user: crate::entities_v2::user::UserPublicResponse,
    pub review_reasons: Vec<AudienceReviewReason>,
}

#[derive(Debug, Serialize, Default)]
pub struct HistorySummary {
    pub eligible_trace_count: i64,
    pub already_accessible_trace_count: i64,
    pub newly_shareable_trace_count: i64,
    pub newly_shareable_normal_trace_count: i64,
    pub newly_shareable_sensitive_trace_count: i64,
    pub review_required: bool,
}

#[derive(Serialize, QueryableByName)]
pub struct AudiencePost {
    #[diesel(sql_type = SqlUuid)]
    pub post_id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    pub trace_id: Uuid,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    pub journal_id: Option<Uuid>,
    #[diesel(sql_type = SqlUuid)]
    pub author_user_id: Uuid,
    #[diesel(sql_type = Text)]
    pub title: String,
    #[diesel(sql_type = Text)]
    pub content: String,
    #[diesel(sql_type = Text)]
    pub sharing_sensitivity: String,
    #[diesel(sql_type = Nullable<Timestamp>)]
    pub publishing_date: Option<NaiveDateTime>,
    #[diesel(sql_type = Bool)]
    pub has_access: bool,
}

#[derive(Deserialize)]
pub struct AudienceNameDto {
    pub name: String,
}

#[derive(Deserialize)]
pub struct AudienceListQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    #[serde(default)]
    pub include_archived: bool,
}

fn enabled() -> bool {
    true
}

#[derive(Deserialize)]
pub struct AddMemberDto {
    #[serde(default = "enabled")]
    pub future_access_enabled: bool,
}

#[derive(Deserialize)]
pub struct UpdateMemberDto {
    pub future_access_enabled: bool,
}

#[derive(Deserialize)]
pub struct RemoveMemberQuery {
    #[serde(default = "enabled")]
    pub stop_future: bool,
    #[serde(default)]
    pub revoke_history: bool,
}

#[derive(Deserialize, Default)]
pub struct RevokeHistoryQuery {
    #[serde(default)]
    pub revoke_history: bool,
}

#[derive(Deserialize)]
pub struct HistoryDecisionDto {
    pub decision: JournalHistoryDecision,
    pub post_ids: Option<Vec<Uuid>>,
}

#[derive(Serialize)]
pub struct MemberChange {
    pub member: AudienceMember,
    pub revoked_post_count: i64,
}

#[derive(Debug, Serialize)]
pub struct HistoryDecisionResult {
    pub member: AudienceMember,
    pub newly_shared_post_count: i64,
}

#[derive(Serialize)]
pub struct AudienceChange {
    pub audience: Audience,
    pub newly_shared_grant_count: i64,
    pub revoked_grant_count: i64,
}
