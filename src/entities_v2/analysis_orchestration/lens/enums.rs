use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LensProcessingState {
    OutOfSync,
    InSync,
}

impl LensProcessingState {
    pub fn to_db(self) -> &'static str {
        match self {
            LensProcessingState::OutOfSync => "OUT_OF_SYNC",
            LensProcessingState::InSync => "IN_SYNC",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "IN_SYNC" | "in_sync" => LensProcessingState::InSync,
            _ => LensProcessingState::OutOfSync,
        }
    }
}
