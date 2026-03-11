use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JournalGrantScope {
    AllAcceptedFollowers,
}

impl JournalGrantScope {
    pub fn to_db(self) -> &'static str {
        match self {
            JournalGrantScope::AllAcceptedFollowers => "ALL_ACCEPTED_FOLLOWERS",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "ALL_ACCEPTED_FOLLOWERS" => Some(JournalGrantScope::AllAcceptedFollowers),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JournalGrantAccessLevel {
    Read,
}

impl JournalGrantAccessLevel {
    pub fn to_db(self) -> &'static str {
        match self {
            JournalGrantAccessLevel::Read => "READ",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "READ" => JournalGrantAccessLevel::Read,
            _ => JournalGrantAccessLevel::Read,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JournalGrantStatus {
    Active,
    Revoked,
}

impl JournalGrantStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            JournalGrantStatus::Active => "ACTIVE",
            JournalGrantStatus::Revoked => "REVOKED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "REVOKED" => JournalGrantStatus::Revoked,
            _ => JournalGrantStatus::Active,
        }
    }
}
