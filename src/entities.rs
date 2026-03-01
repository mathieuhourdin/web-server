pub mod interaction;
pub mod resource;
pub mod resource_relation;

pub mod error {
    pub use crate::entities_v2::platform_infra::error::*;
}

pub mod session {
    pub use crate::entities_v2::platform_infra::session::*;
}

pub mod transcription {
    pub use crate::entities_v2::platform_infra::transcription::*;
}

pub mod user {
    pub use crate::entities_v2::platform_infra::user::*;
}
