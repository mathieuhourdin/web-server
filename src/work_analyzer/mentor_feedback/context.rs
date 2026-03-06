use serde::Serialize;
use uuid::Uuid;

use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::entities_v2::user::User;
use crate::work_analyzer::analysis_context::AnalysisContext;
use crate::work_analyzer::period_summary::{build_day_context, DaySummaryPromptContext};

#[derive(Debug, Serialize)]
pub struct MentorFeedbackPromptContext {
    pub analysis_id: Uuid,
    pub mentor: MentorProfileContextItem,
    pub summary_context: DaySummaryPromptContext,
}

#[derive(Debug, Serialize)]
pub struct MentorProfileContextItem {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub biography: Option<String>,
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

    Ok(MentorFeedbackPromptContext {
        analysis_id: analysis.id,
        mentor: MentorProfileContextItem {
            id: mentor_user.id,
            first_name: mentor_user.first_name,
            last_name: mentor_user.last_name,
            biography: mentor_user.biography,
        },
        summary_context,
    })
}
