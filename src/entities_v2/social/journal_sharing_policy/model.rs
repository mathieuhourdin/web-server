use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities_v2::{journal::JournalSharingMode, user::UserPublicResponse};

pub use super::enums::{
    JournalHistoryDecision, JournalHistoryReviewState, JournalSharingPolicyStatus,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JournalSharingPolicy {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub owner_user_id: Uuid,
    pub grantee_user_id: Uuid,
    pub status: JournalSharingPolicyStatus,
    pub default_future_access_enabled: bool,
    pub history_review_state: JournalHistoryReviewState,
    pub history_decision: Option<JournalHistoryDecision>,
    pub history_reviewed_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NewJournalSharingPolicyDto {
    pub grantee_user_id: Uuid,
    pub status: Option<JournalSharingPolicyStatus>,
    pub default_future_access_enabled: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct UpdateJournalSharingPolicyDto {
    pub status: Option<JournalSharingPolicyStatus>,
    pub default_future_access_enabled: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct JournalSharingPolicyHistoryDecisionDto {
    pub decision: JournalHistoryDecision,
    pub post_ids: Option<Vec<Uuid>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalSharingPolicyReviewReason {
    FutureDefault,
    History,
}

#[derive(Serialize, Debug, Clone)]
pub struct JournalSharingPolicyReviewJournal {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub sharing_mode: JournalSharingMode,
}

#[derive(Serialize)]
pub struct JournalSharingPolicyPendingReview {
    pub policy: JournalSharingPolicy,
    pub journal: JournalSharingPolicyReviewJournal,
    pub grantee: UserPublicResponse,
    pub review_reasons: Vec<JournalSharingPolicyReviewReason>,
}
