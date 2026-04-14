use crate::entities_v2::error::PpdcError;
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::entities_v2::message::{Message, MentorFeedbackMetadata};
use crate::entities_v2::user::User;
use crate::openai_handler::{GptReasoningEffort, GptRequestConfig, GptVerbosity};
use crate::work_analyzer::analysis_context::AnalysisContext;
use serde::{Deserialize, Serialize};

use super::context::build as build_context;
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
    context: super::context::MentorFeedbackPromptContext,
}

pub async fn send(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
) -> Result<Option<Message>, PpdcError> {
    let recipient_user = User::find(&analysis.user_id, &context.pool)?;
    let Some(mentor_id) = recipient_user.mentor_id else {
        return Ok(None);
    };

    let prompt_context = build_context(context, analysis)?;
    let mentor_name = format!(
        "{} {}",
        prompt_context.mentor.first_name, prompt_context.mentor.last_name
    );
    let mentor_biography = prompt_context.mentor.biography.clone();
    let system_prompt = include_str!("system.md").to_string();
    let schema: serde_json::Value = serde_json::from_str(include_str!("schema.json"))?;
    let user_prompt = serde_json::to_string_pretty(&MentorFeedbackPromptInput {
        mentor_name,
        mentor_biography,
        context: prompt_context,
    })?;

    let feedback = GptRequestConfig::new(
        "gpt-5.1".to_string(),
        system_prompt,
        user_prompt,
        Some(schema),
        Some(context.analysis_id),
    )
    .with_reasoning_effort(GptReasoningEffort::Low)
    .with_verbosity(GptVerbosity::Low)
    .with_display_name("Mentor Feedback / Day Feedback")
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
