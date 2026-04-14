use serde::Serialize;
use uuid::Uuid;

use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::entities_v2::message::{MentorFeedbackMetadata, Message};
use crate::entities_v2::user::User;
use crate::work_analyzer::analysis_context::AnalysisContext;
use crate::work_analyzer::period_summary::{build_day_context, DaySummaryPromptContext};

#[derive(Debug, Serialize)]
pub struct MentorFeedbackPromptContext {
    pub analysis_id: Uuid,
    pub mentor: MentorProfileContextItem,
    pub summary_context: DaySummaryPromptContext,
    pub recent_feedback_metadata: Vec<RecentMentorFeedbackMetadataContextItem>,
    pub recent_feedbacks: Vec<RecentMentorFeedbackContextItem>,
}

#[derive(Debug, Serialize)]
pub struct MentorProfileContextItem {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub biography: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RecentMentorFeedbackMetadataContextItem {
    pub metadata: MentorFeedbackMetadata,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct RecentMentorFeedbackContextItem {
    pub title: String,
    pub content: String,
    pub metadata: Option<MentorFeedbackMetadata>,
    pub created_at: chrono::NaiveDateTime,
}

pub fn build(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
) -> Result<MentorFeedbackPromptContext, PpdcError> {
    let recipient_user = User::find(&analysis.user_id, &context.pool)?;
    let mentor_id = recipient_user.mentor_id.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("User {} does not have a mentor assigned", recipient_user.id),
        )
    })?;
    let mentor_user = User::find(&mentor_id, &context.pool)?;
    let summary_context = build_day_context(context, analysis)?;
    let recent_feedbacks = Message::find_recent_mentor_feedbacks_for_user(recipient_user.id, 30, &context.pool)?;
    let recent_feedback_metadata = recent_feedbacks
        .iter()
        .filter_map(|message| {
            message
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.mentor_feedback.clone())
                .map(|metadata| RecentMentorFeedbackMetadataContextItem {
                    metadata,
                    created_at: message.created_at,
                })
        })
        .take(15)
        .collect::<Vec<_>>();
    let recent_feedbacks = recent_feedbacks
        .into_iter()
        .take(5)
        .map(|message| RecentMentorFeedbackContextItem {
            title: message.title,
            content: message.content,
            metadata: message.metadata.and_then(|metadata| metadata.mentor_feedback),
            created_at: message.created_at,
        })
        .collect::<Vec<_>>();

    Ok(MentorFeedbackPromptContext {
        analysis_id: analysis.id,
        mentor: MentorProfileContextItem {
            id: mentor_user.id,
            first_name: mentor_user.first_name,
            last_name: mentor_user.last_name,
            biography: mentor_user.biography,
        },
        summary_context,
        recent_feedback_metadata,
        recent_feedbacks,
    })
}
