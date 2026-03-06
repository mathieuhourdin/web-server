use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Reference {
    pub id: Uuid,
    pub tag_id: i32,
    pub trace_mirror_id: Uuid,
    pub landmark_id: Option<Uuid>,
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub mention: String,
    pub reference_type: ReferenceType,
    pub context_tags: Vec<String>,
    pub reference_variants: Vec<String>,
    pub parent_reference_id: Option<Uuid>,
    pub is_user_specific: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewReference {
    pub tag_id: i32,
    pub trace_mirror_id: Uuid,
    pub landmark_id: Option<Uuid>,
    pub landscape_analysis_id: Uuid,
    pub user_id: Uuid,
    pub mention: String,
    pub reference_type: ReferenceType,
    pub context_tags: Vec<String>,
    pub reference_variants: Vec<String>,
    pub parent_reference_id: Option<Uuid>,
    pub is_user_specific: bool,
}

impl NewReference {
    pub fn new(
        tag_id: i32,
        trace_mirror_id: Uuid,
        landmark_id: Option<Uuid>,
        landscape_analysis_id: Uuid,
        user_id: Uuid,
        mention: String,
        reference_type: ReferenceType,
        context_tags: Vec<String>,
        reference_variants: Vec<String>,
        parent_reference_id: Option<Uuid>,
        is_user_specific: bool,
    ) -> Self {
        Self {
            tag_id,
            trace_mirror_id,
            landmark_id,
            landscape_analysis_id,
            user_id,
            mention,
            reference_type,
            context_tags,
            reference_variants,
            parent_reference_id,
            is_user_specific,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceType {
    #[serde(alias = "PROPER_NAME")]
    ProperName,
    #[serde(alias = "NAMED_DESC")]
    NamedDesc,
    #[serde(alias = "DEICTIC_DESC")]
    DeicticDesc,
    #[serde(alias = "PLAIN_DESC")]
    PlainDesc,
    #[serde(alias = "NICKNAME")]
    Nickname,
}

impl ReferenceType {
    pub fn to_db(self) -> &'static str {
        match self {
            ReferenceType::ProperName => "PROPER_NAME",
            ReferenceType::NamedDesc => "NAMED_DESC",
            ReferenceType::DeicticDesc => "DEICTIC_DESC",
            ReferenceType::PlainDesc => "PLAIN_DESC",
            ReferenceType::Nickname => "NICKNAME",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "PROPER_NAME" | "proper_name" => ReferenceType::ProperName,
            "NAMED_DESC" | "named_desc" => ReferenceType::NamedDesc,
            "DEICTIC_DESC" | "deictic_desc" => ReferenceType::DeicticDesc,
            "NICKNAME" | "nickname" => ReferenceType::Nickname,
            _ => ReferenceType::PlainDesc,
        }
    }
}
