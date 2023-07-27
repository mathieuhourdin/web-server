use serde::{Serialize, Deserialize};
use std::time::{SystemTime, Duration};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    user_id: Option<String>,
    created_at: SystemTime,
    expires_at: SystemTime,
    authenticated: bool,
}

impl Session {
    pub fn new() -> Session {
        Session {
            id: Uuid::new_v4(),
            user_id: None,
            created_at: SystemTime::now(),
            expires_at: SystemTime::now() + Duration::from_secs(3600),
            authenticated: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.expires_at > SystemTime::now()
    }
}
