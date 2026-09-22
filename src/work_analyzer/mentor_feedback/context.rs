use serde::Serialize;
use uuid::Uuid;

use crate::entities_v2::analysis_summary::AnalysisSummary;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::entities_v2::message::{MentorFeedbackMetadata, Message};
use crate::entities_v2::user::User;
use crate::work_analyzer::analysis_context::AnalysisContext;
use crate::work_analyzer::period_summary::{
    build_day_context, build_week_context, DaySummaryPromptContext, WeekSummaryPromptContext,
};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MentorFeedbackPeriodKind {
    Daily,
    Weekly,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum MentorFeedbackSummaryContext {
    Daily(DaySummaryPromptContext),
    Weekly(WeekSummaryPromptContext),
}

#[derive(Debug, Serialize)]
pub struct MentorFeedbackPromptContext {
    pub analysis_id: Uuid,
    pub period_kind: MentorFeedbackPeriodKind,
    pub mentor: MentorProfileContextItem,
    pub period_summary: PeriodSummaryContextItem,
    pub summary_context: MentorFeedbackSummaryContext,
    pub recent_feedback_metadata: Vec<RecentMentorFeedbackMetadataContextItem>,
    pub recent_feedbacks: Vec<RecentMentorFeedbackContextItem>,
}

#[derive(Debug, Serialize)]
pub struct PeriodSummaryContextItem {
    pub title: String,
    pub short_content: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct MentorProfileContextItem {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub biography: Option<String>,
    pub mentor_specific_prompt: Option<String>,
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

pub fn build_day(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
    summary: &AnalysisSummary,
) -> Result<MentorFeedbackPromptContext, PpdcError> {
    build(
        context,
        analysis,
        summary,
        MentorFeedbackPeriodKind::Daily,
        MentorFeedbackSummaryContext::Daily(build_day_context(context, analysis)?),
    )
}

pub fn build_week(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
    summary: &AnalysisSummary,
) -> Result<MentorFeedbackPromptContext, PpdcError> {
    build(
        context,
        analysis,
        summary,
        MentorFeedbackPeriodKind::Weekly,
        MentorFeedbackSummaryContext::Weekly(build_week_context(context, analysis)?),
    )
}

fn build(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
    summary: &AnalysisSummary,
    period_kind: MentorFeedbackPeriodKind,
    summary_context: MentorFeedbackSummaryContext,
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
    let recent_feedbacks =
        Message::find_recent_mentor_feedbacks_for_user(recipient_user.id, 30, &context.pool)?;
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
            metadata: message
                .metadata
                .and_then(|metadata| metadata.mentor_feedback),
            created_at: message.created_at,
        })
        .collect::<Vec<_>>();

    Ok(MentorFeedbackPromptContext {
        analysis_id: analysis.id,
        period_kind,
        mentor: MentorProfileContextItem {
            id: mentor_user.id,
            first_name: mentor_user.first_name,
            last_name: mentor_user.last_name,
            biography: mentor_user.biography,
            mentor_specific_prompt: mentor_user.mentor_specific_prompt,
        },
        period_summary: PeriodSummaryContextItem {
            title: summary.title.clone(),
            short_content: summary.short_content.clone(),
            content: summary.content.clone(),
        },
        summary_context,
        recent_feedback_metadata,
        recent_feedbacks,
    })
}
