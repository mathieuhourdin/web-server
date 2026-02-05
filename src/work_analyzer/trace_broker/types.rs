use crate::entities::error::{ErrorType, PpdcError};
use serde::{Deserialize, Serialize};
use serde::de::{self, Deserializer};
use serde::Serializer;
use crate::entities::resource::maturing_state::MaturingState;
use crate::entities_v2::landmark::LandmarkType;
use crate::work_analyzer::trace_broker::traits::ProcessorContext;
use crate::work_analyzer::trace_broker::landmark_resource_processor;



#[derive(Debug, Clone, Copy)]
pub enum IdentityState {
    Identified,
    Stub,
    Discard,
}

impl IdentityState {
    pub fn to_string(&self) -> String {
        match self {
            IdentityState::Identified => "identified".to_string(),
            IdentityState::Stub => "stub".to_string(),
            IdentityState::Discard => "discard".to_string(),
        }
    }
    pub fn from_string(string: &str) -> Result<IdentityState, PpdcError> {
        match string {
            "identified" => Ok(IdentityState::Identified),
            "stub" => Ok(IdentityState::Stub),
            "discard" => Ok(IdentityState::Discard),
            _ => Err(PpdcError::new(400, ErrorType::ApiError, "Invalid identity state".to_string())),
        }
    }
}

impl From<IdentityState> for MaturingState {
    fn from(identity_state: IdentityState) -> Self {
        match identity_state {
            IdentityState::Identified => MaturingState::Finished,
            IdentityState::Stub => MaturingState::Draft,
            IdentityState::Discard => MaturingState::Trashed,
        }
    }
}
impl From<MaturingState> for IdentityState {
    fn from(maturing_state: MaturingState) -> Self {
        match maturing_state {
            MaturingState::Finished => IdentityState::Identified,
            MaturingState::Review => IdentityState::Stub,
            MaturingState::Trashed => IdentityState::Discard,
            MaturingState::Draft => IdentityState::Discard,
        }
    }
}
impl Serialize for IdentityState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl<'de> Deserialize<'de> for IdentityState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        IdentityState::from_string(&s).map_err(|_err| de::Error::custom("unknown identity state"))
    }
}