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

    /// Whether a trace in this status may back a published post.
    /// Per `doc/publication.md`: only finalized traces; drafts have no post and
    /// archived traces give archived posts.
    pub fn permits_published_post(self) -> bool {
        matches!(self, TraceStatus::Finalized)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceSharingSensitivity {
    Normal,
    Sensitive,
}

impl TraceSharingSensitivity {
    pub fn to_db(self) -> &'static str {
        match self {
            TraceSharingSensitivity::Normal => "NORMAL",
            TraceSharingSensitivity::Sensitive => "SENSITIVE",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "SENSITIVE" | "sensitive" => TraceSharingSensitivity::Sensitive,
            _ => TraceSharingSensitivity::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mirrors the trace row of the table in doc/publication.md.
    #[test]
    fn permits_published_post_matches_publication_model() {
        assert!(!TraceStatus::Draft.permits_published_post());
        assert!(TraceStatus::Finalized.permits_published_post());
        assert!(!TraceStatus::Archived.permits_published_post());
    }
}
