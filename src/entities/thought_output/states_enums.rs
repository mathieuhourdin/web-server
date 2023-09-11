use crate::entities::{error::{PpdcError, ErrorType}};

pub enum MaturingState {
    Idea,
    InProgress,
    Review,
    Finished
}
impl MaturingState {
    pub fn to_str(&self) -> &str {
        match self {
            MaturingState::Idea => "idea",
            MaturingState::InProgress => "prgs",
            MaturingState::Review => "rvew",
            MaturingState::Finished => "fnsh"
        }
    }

    pub fn from_str(str_representation: &str) -> Result<Self, PpdcError> {
        let return_value = match str_representation {
            "idea" => MaturingState::Idea,
            "prgs" => MaturingState::InProgress,
            "rvew" => MaturingState::Review,
            "fnsh" => MaturingState::Finished,
            _ => return Err(PpdcError::new(400, ErrorType::ApiError, "maturing_state unknown value".to_string()))
        };
        Ok(return_value)
    }
}

