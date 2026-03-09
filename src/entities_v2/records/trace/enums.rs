use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceType {
    BioTrace,
    WorkspaceTrace,
    UserTrace,
    HighLevelProjectsDefinition,
}

impl TraceType {
    pub fn to_db(self) -> &'static str {
        match self {
            TraceType::UserTrace => "USER_TRACE",
            TraceType::BioTrace => "BIO_TRACE",
            TraceType::WorkspaceTrace => "WORKSPACE_TRACE",
            TraceType::HighLevelProjectsDefinition => "HIGH_LEVEL_PROJECTS_DEFINITION",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "BIO_TRACE" | "btrc" | "BTRC" => TraceType::BioTrace,
            "WORKSPACE_TRACE" | "wtrc" | "WTRC" => TraceType::WorkspaceTrace,
            "HIGH_LEVEL_PROJECTS_DEFINITION" | "hlpd" | "HLPD" => {
                TraceType::HighLevelProjectsDefinition
            }
            "USER_TRACE" | "trce" | "TRCE" => TraceType::UserTrace,
            _ => TraceType::UserTrace,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceStatus {
    Draft,
    Finalized,
    Archived,
}

impl TraceStatus {
    pub fn to_db(self) -> &'static str {
        match self {
            TraceStatus::Draft => "DRAFT",
            TraceStatus::Finalized => "FINALIZED",
            TraceStatus::Archived => "ARCHIVED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "FINALIZED" => TraceStatus::Finalized,
            "ARCHIVED" => TraceStatus::Archived,
            _ => TraceStatus::Draft,
        }
    }
}
