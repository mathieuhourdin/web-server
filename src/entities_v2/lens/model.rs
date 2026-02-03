use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Lens {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub description: String,
    pub fork_landscape_id: Option<Uuid>,
    pub current_landscape_id: Option<Uuid>,
    pub current_trace_id: Uuid,
    pub current_state_date: NaiveDateTime,
    pub model_version: String,
    pub autoplay: bool,
    pub is_primary: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

pub struct NewLens {
    pub name: String,
    pub description: String,
    pub fork_landscape_id: Option<Uuid>,
    pub current_trace_id: Uuid,
    pub current_state_date: NaiveDateTime,
    pub current_landscape_id: Uuid,
    pub model_version: String,
    pub autoplay: bool,
    pub is_primary: bool,
    pub user_id: Uuid,
}

impl NewLens {
    pub fn new(payload: NewLensDto, current_trace_id: Uuid, current_landscape_id: Uuid, user_id: Uuid) -> NewLens {
        NewLens {
            name: payload.name.unwrap_or_default(),
            description: payload.description.unwrap_or_default(),
            fork_landscape_id: payload.fork_landscape_id,
            current_trace_id: current_trace_id,
            current_state_date: payload.current_state_date.unwrap_or_default(),
            current_landscape_id: current_landscape_id,
            model_version: payload.model_version.unwrap_or_default(),
            autoplay: payload.autoplay.unwrap_or_default(),
            is_primary: payload.is_primary.unwrap_or_default(),
            user_id: user_id,
        }
    }
}

#[derive(Deserialize)]
pub struct NewLensDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub fork_landscape_id: Option<Uuid>,
    pub current_trace_id: Uuid,
    pub current_state_date: Option<NaiveDateTime>,
    pub model_version: Option<String>,
    pub autoplay: Option<bool>,
    pub is_primary: Option<bool>,
}
