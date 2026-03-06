use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Lens {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub processing_state: LensProcessingState,
    pub fork_landscape_id: Option<Uuid>,
    pub current_landscape_id: Option<Uuid>,
    pub target_trace_id: Option<Uuid>,
    pub autoplay: bool,
}

pub struct NewLens {
    pub processing_state: LensProcessingState,
    pub fork_landscape_id: Option<Uuid>,
    pub target_trace_id: Option<Uuid>,
    pub current_landscape_id: Option<Uuid>,
    pub autoplay: bool,
    pub user_id: Uuid,
}

impl NewLens {
    pub fn new(payload: NewLensDto, current_landscape_id: Option<Uuid>, user_id: Uuid) -> NewLens {
        NewLens {
            processing_state: payload
                .processing_state
                .unwrap_or(LensProcessingState::InSync),
            fork_landscape_id: payload.fork_landscape_id,
            target_trace_id: payload.target_trace_id,
            current_landscape_id: current_landscape_id,
            autoplay: payload.autoplay.unwrap_or_default(),
            user_id: user_id,
        }
    }
}

#[derive(Deserialize)]
pub struct NewLensDto {
    pub processing_state: Option<LensProcessingState>,
    pub fork_landscape_id: Option<Uuid>,
    pub target_trace_id: Option<Uuid>,
    pub autoplay: Option<bool>,
}
