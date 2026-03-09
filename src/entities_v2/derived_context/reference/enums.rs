use serde::{Deserialize, Serialize};

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
