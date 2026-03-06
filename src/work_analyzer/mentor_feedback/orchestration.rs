use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::message::Message;

use super::persistence::create_mentor_feedback;

pub async fn send(
    analysis_id: Uuid,
    sender_user_id: Uuid,
    title: String,
    content: String,
    pool: &DbPool,
) -> Result<Message, PpdcError> {
    create_mentor_feedback(analysis_id, sender_user_id, title, content, pool)
}
