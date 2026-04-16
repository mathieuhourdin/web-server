use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::{
    landscape_analysis::LandscapeAnalysis,
    message::{MentorFeedbackMetadata, Message, MessageMetadata, MessageType, NewMessage},
};

pub fn create_mentor_feedback(
    analysis_id: Uuid,
    sender_user_id: Uuid,
    title: String,
    content: String,
    metadata: MentorFeedbackMetadata,
    pool: &DbPool,
) -> Result<Message, PpdcError> {
    let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, pool)?;
    NewMessage {
        sender_user_id,
        recipient_user_id: analysis.user_id,
        landscape_analysis_id: Some(analysis_id),
        trace_id: None,
        post_id: None,
        reply_to_message_id: None,
        message_type: MessageType::MentorFeedback,
        processing_state: crate::entities_v2::message::MessageProcessingState::Processed,
        title,
        content,
        attachment_type: None,
        attachment: None,
        metadata: Some(MessageMetadata::mentor_feedback(metadata)),
    }
    .create(pool)
}
