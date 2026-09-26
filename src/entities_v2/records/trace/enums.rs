use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceType {
    BioTrace,
    WorkspaceTrace,
    UserTrace,
    LinkedTrace,
    TraceComplement,
    HighLevelProjectsDefinition,
}

impl TraceType {
    pub fn to_db(self) -> &'static str {
        match self {
            TraceType::UserTrace => "USER_TRACE",
            TraceType::LinkedTrace => "LINKED_TRACE",
            TraceType::TraceComplement => "TRACE_COMPLEMENT",
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
            "LINKED_TRACE" => TraceType::LinkedTrace,
            "TRACE_COMPLEMENT" => TraceType::TraceComplement,
            _ => TraceType::UserTrace,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceComplementAudienceMode {
    Independent,
    Parent,
}

impl Default for TraceComplementAudienceMode {
    fn default() -> Self {
        Self::Independent
    }
}

impl TraceComplementAudienceMode {
    pub fn to_db(self) -> &'static str {
        match self {
            Self::Independent => "INDEPENDENT",
            Self::Parent => "PARENT",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "PARENT" | "parent" => Self::Parent,
            _ => Self::Independent,
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

    #[test]
    fn trace_complement_uses_the_api_and_database_codes() {
        assert_eq!(TraceType::TraceComplement.to_db(), "TRACE_COMPLEMENT");
        assert_eq!(
            TraceType::from_db("TRACE_COMPLEMENT"),
            TraceType::TraceComplement
        );
        assert_eq!(
            serde_json::to_string(&TraceType::TraceComplement).unwrap(),
            "\"trace_complement\""
        );
    }

    #[test]
    fn trace_complement_audience_modes_use_stable_codes() {
        assert_eq!(
            TraceComplementAudienceMode::Independent.to_db(),
            "INDEPENDENT"
        );
        assert_eq!(TraceComplementAudienceMode::Parent.to_db(), "PARENT");
        assert_eq!(
            serde_json::to_string(&TraceComplementAudienceMode::Independent).unwrap(),
            "\"independent\""
        );
        assert_eq!(
            serde_json::to_string(&TraceComplementAudienceMode::Parent).unwrap(),
            "\"parent\""
        );
    }
}
