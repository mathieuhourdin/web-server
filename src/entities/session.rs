use serde::{Serialize, Deserialize};
use std::time::{SystemTime, Duration};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    created_at: SystemTime,
    pub expires_at: SystemTime,
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

    pub fn set_user_id(&mut self, user_id: Uuid) {
        self.user_id = Some(user_id);
    }
}
