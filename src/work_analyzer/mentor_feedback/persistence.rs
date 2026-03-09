use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::{
    landscape_analysis::LandscapeAnalysis,
    message::{Message, MessageType, NewMessage, NewMessageDto},
};

pub fn create_mentor_feedback(
    analysis_id: Uuid,
    sender_user_id: Uuid,
    title: String,
    content: String,
    pool: &DbPool,
) -> Result<Message, PpdcError> {
    let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, pool)?;
    NewMessage::new(
        NewMessageDto {
            recipient_user_id: analysis.user_id,
            landscape_analysis_id: Some(analysis_id),
            trace_id: None,
            message_type: Some(MessageType::MentorFeedback),
            title: Some(title),
            content,
            attachment_type: None,
            attachment: None,
        },
        sender_user_id,
    )
    .create(pool)
}
