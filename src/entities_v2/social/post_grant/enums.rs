use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PostGrantScope {
    AllAcceptedFollowers,
}

impl PostGrantScope {
    pub fn to_db(self) -> &'static str {
        match self {
            PostGrantScope::AllAcceptedFollowers => "ALL_ACCEPTED_FOLLOWERS",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "ALL_ACCEPTED_FOLLOWERS" => Some(PostGrantScope::AllAcceptedFollowers),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PostGrantAccessLevel {
    Read,
}

impl PostGrantAccessLevel {
    pub fn to_db(self) -> &'static str {
        match self {
            PostGrantAccessLevel::Read => "READ",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "READ" => PostGrantAccessLevel::Read,
            _ => PostGrantAccessLevel::Read,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PostGrantStatus {
    Active,
    Revoked,
}

impl PostGrantStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            PostGrantStatus::Active => "ACTIVE",
            PostGrantStatus::Revoked => "REVOKED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "REVOKED" => PostGrantStatus::Revoked,
            _ => PostGrantStatus::Active,
        }
    }
}
