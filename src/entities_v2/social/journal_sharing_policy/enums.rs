use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalSharingPolicyStatus {
    Suggested,
    Active,
    Disabled,
    Dismissed,
    Revoked,
}

impl JournalSharingPolicyStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            JournalSharingPolicyStatus::Suggested => "SUGGESTED",
            JournalSharingPolicyStatus::Active => "ACTIVE",
            JournalSharingPolicyStatus::Disabled => "DISABLED",
            JournalSharingPolicyStatus::Dismissed => "DISMISSED",
            JournalSharingPolicyStatus::Revoked => "REVOKED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "ACTIVE" => JournalSharingPolicyStatus::Active,
            "DISABLED" => JournalSharingPolicyStatus::Disabled,
            "DISMISSED" => JournalSharingPolicyStatus::Dismissed,
            "REVOKED" => JournalSharingPolicyStatus::Revoked,
            _ => JournalSharingPolicyStatus::Suggested,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalHistoryReviewState {
    NotStarted,
    Unreviewed,
    Reviewed,
}

impl JournalHistoryReviewState {
    pub fn to_db(self) -> &'static str {
        match self {
            JournalHistoryReviewState::NotStarted => "NOT_STARTED",
            JournalHistoryReviewState::Unreviewed => "UNREVIEWED",
            JournalHistoryReviewState::Reviewed => "REVIEWED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "UNREVIEWED" => JournalHistoryReviewState::Unreviewed,
            "REVIEWED" => JournalHistoryReviewState::Reviewed,
            _ => JournalHistoryReviewState::NotStarted,
        }
    }
}

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
            JournalHistoryDecision::None => "NONE",
            JournalHistoryDecision::AllNormal => "ALL_NORMAL",
            JournalHistoryDecision::AllIncludingSensitive => "ALL_INCLUDING_SENSITIVE",
            JournalHistoryDecision::UserSelected => "USER_SELECTED",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "NONE" => Some(JournalHistoryDecision::None),
            "ALL_NORMAL" | "NORMAL_ONLY" => Some(JournalHistoryDecision::AllNormal),
            "ALL_INCLUDING_SENSITIVE" | "ALL" => {
                Some(JournalHistoryDecision::AllIncludingSensitive)
            }
            "USER_SELECTED" | "SPECIFIC" => Some(JournalHistoryDecision::UserSelected),
            _ => None,
        }
    }
}
