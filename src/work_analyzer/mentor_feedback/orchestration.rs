use crate::entities_v2::analysis_summary::AnalysisSummary;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::entities_v2::message::{MentorFeedbackMetadata, Message};
use crate::entities_v2::user::User;
use crate::openai_handler::{GptReasoningEffort, GptRequestConfig, GptVerbosity};
use crate::work_analyzer::analysis_context::AnalysisContext;
use crate::work_analyzer::MENTOR_OPENAI_MODEL;
use serde::{Deserialize, Serialize};

use super::context::{build_day, build_week, MentorFeedbackPromptContext};
use super::persistence::create_mentor_feedback;

#[derive(Debug, Deserialize)]
struct MentorFeedbackDraft {
    title: String,
    content: String,
    metadata: MentorFeedbackMetadata,
}

#[derive(Debug, Serialize)]
struct MentorFeedbackPromptInput {
    mentor_name: String,
    mentor_biography: Option<String>,
    mentor_specific_prompt: Option<String>,
    context: super::context::MentorFeedbackPromptContext,
}

pub async fn send_day(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
    summary: &AnalysisSummary,
) -> Result<Option<Message>, PpdcError> {
    send(
        context,
        analysis,
        build_day(context, analysis, summary)?,
        include_str!("system.md"),
        "Mentor Feedback / Day Feedback",
    )
    .await
}

pub async fn send_week(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
    summary: &AnalysisSummary,
) -> Result<Option<Message>, PpdcError> {
    send(
        context,
        analysis,
        build_week(context, analysis, summary)?,
        include_str!("week_system.md"),
        "Mentor Feedback / Week Feedback",
    )
    .await
}

async fn send(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
    prompt_context: MentorFeedbackPromptContext,
    system_prompt: &str,
    display_name: &str,
) -> Result<Option<Message>, PpdcError> {
    let recipient_user = User::find(&analysis.user_id, &context.pool)?;
    let Some(mentor_id) = recipient_user.mentor_id else {
        return Ok(None);
    };

    let mentor_name = format!(
        "{} {}",
        prompt_context.mentor.first_name, prompt_context.mentor.last_name
    );
    let mentor_biography = prompt_context.mentor.biography.clone();
    let mentor_specific_prompt = prompt_context.mentor.mentor_specific_prompt.clone();
    let schema: serde_json::Value = serde_json::from_str(include_str!("schema.json"))?;
    let user_prompt = serde_json::to_string_pretty(&MentorFeedbackPromptInput {
        mentor_name,
        mentor_biography,
        mentor_specific_prompt,
        context: prompt_context,
    })?;

    let feedback = GptRequestConfig::new(
        MENTOR_OPENAI_MODEL.to_string(),
        system_prompt.to_string(),
        user_prompt,
        Some(schema),
        Some(context.analysis_id),
    )
    .with_reasoning_effort(GptReasoningEffort::Low)
    .with_verbosity(GptVerbosity::Low)
    .with_display_name(display_name)
    .execute::<MentorFeedbackDraft>()
    .await?;

    Ok(Some(create_mentor_feedback(
        analysis.id,
        mentor_id,
        feedback.title,
        feedback.content,
        feedback.metadata,
        &context.pool,
    )?))
}
