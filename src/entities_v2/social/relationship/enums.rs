use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationshipType {
    Follow,
}

impl RelationshipType {
    pub fn to_db(self) -> &'static str {
        match self {
            RelationshipType::Follow => "FOLLOW",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "FOLLOW" => RelationshipType::Follow,
            _ => RelationshipType::Follow,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationshipStatus {
    Pending,
    Accepted,
    Rejected,
    Blocked,
}

impl RelationshipStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            RelationshipStatus::Pending => "PENDING",
            RelationshipStatus::Accepted => "ACCEPTED",
            RelationshipStatus::Rejected => "REJECTED",
            RelationshipStatus::Blocked => "BLOCKED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "ACCEPTED" => RelationshipStatus::Accepted,
            "REJECTED" => RelationshipStatus::Rejected,
            "BLOCKED" => RelationshipStatus::Blocked,
            _ => RelationshipStatus::Pending,
        }
    }
}
