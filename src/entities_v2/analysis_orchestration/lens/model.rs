use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::resource::maturing_state::MaturingState;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LensProcessingState {
    OutOfSync,
    InSync,
}

impl LensProcessingState {
    pub fn from_maturing_state(state: MaturingState) -> Self {
        match state {
            MaturingState::Finished => LensProcessingState::InSync,
            _ => LensProcessingState::OutOfSync,
        }
    }

    pub fn to_maturing_state(self) -> MaturingState {
        match self {
            LensProcessingState::OutOfSync => MaturingState::Draft,
            LensProcessingState::InSync => MaturingState::Finished,
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
