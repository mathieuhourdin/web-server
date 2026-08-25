use chrono::NaiveDateTime;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, Debug, Clone)]
pub struct UserBlock {
    pub blocker_user_id: Uuid,
    pub blocked_user_id: Uuid,
    pub created_at: NaiveDateTime,
}
