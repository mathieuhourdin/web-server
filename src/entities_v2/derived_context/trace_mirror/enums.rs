use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceMirrorType {
    Note,
    Journal,
    HighLevelProjects,
    Bio,
}

impl TraceMirrorType {
    pub fn to_db(self) -> &'static str {
        match self {
            TraceMirrorType::Note => "NOTE",
            TraceMirrorType::Journal => "JOURNAL",
            TraceMirrorType::HighLevelProjects => "HIGH_LEVEL_PROJECTS",
            TraceMirrorType::Bio => "BIO",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "JOURNAL" | "journal" => TraceMirrorType::Journal,
            "HIGH_LEVEL_PROJECTS" | "high_level_projects" => TraceMirrorType::HighLevelProjects,
            "BIO" | "bio" => TraceMirrorType::Bio,
            _ => TraceMirrorType::Note,
        }
    }
}
