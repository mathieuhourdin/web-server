use chrono::NaiveDateTime;
use serde::Serialize;
use uuid::Uuid;

use crate::entities_v2::analysis_summary::AnalysisSummary;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::entities_v2::message::{Message, MessageType};
use crate::entities_v2::platform_infra::user::User;
use crate::entities_v2::trace::Trace;
use crate::work_analyzer::analysis_context::AnalysisContext;

#[derive(Debug, Serialize)]
pub struct MentorFeedbackPromptContext {
    pub analysis_id: Uuid,
    pub analysis_type: String,
    pub period_start: NaiveDateTime,
    pub period_end: NaiveDateTime,
    pub recipient_user: UserContextItem,
    pub current_state_summary: String,
    pub period_summary: Option<PeriodSummaryContext>,
    pub traces: Vec<TraceContextItem>,
    pub landmarks: Vec<LandmarkContextItem>,
    pub previous_feedback: Vec<PreviousFeedbackContextItem>,
}

#[derive(Debug, Serialize)]
pub struct UserContextItem {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub handle: String,
    pub biography: Option<String>,
    pub high_level_projects_definition: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PeriodSummaryContext {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct TraceContextItem {
    pub id: Uuid,
    pub interaction_date: NaiveDateTime,
    pub trace_type: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct LandmarkContextItem {
    pub id: Uuid,
    pub landmark_type: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct PreviousFeedbackContextItem {
    pub id: Uuid,
    pub sender_user_id: Uuid,
    pub title: String,
    pub content: String,
    pub created_at: NaiveDateTime,
}

pub fn build(
    context: &AnalysisContext,
    analysis: &LandscapeAnalysis,
) -> Result<MentorFeedbackPromptContext, PpdcError> {
    let recipient_user = User::find(&analysis.user_id, &context.pool)?;

    let period_summary = AnalysisSummary::find_for_analysis(analysis.id, &context.pool)?
        .into_iter()
        .find(|summary| summary.summary_type.to_db() == "PERIOD_RECAP")
        .map(|summary| PeriodSummaryContext {
            title: summary.title,
            content: summary.content,
        });

    let traces = analysis
        .get_input_traces(&context.pool)?
        .into_iter()
        .map(|trace| TraceContextItem {
            id: trace.id,
            interaction_date: trace.interaction_date,
            trace_type: trace.trace_type.to_db().to_string(),
            title: trace.title,
            subtitle: trace.subtitle,
            content: trace.content,
        })
        .collect::<Vec<_>>();

    let landmarks = analysis
        .get_landmarks(None, &context.pool)?
        .into_iter()
        .map(|landmark| LandmarkContextItem {
            id: landmark.id,
            landmark_type: landmark.landmark_type.to_code().to_string(),
            title: landmark.title,
            subtitle: landmark.subtitle,
            content: landmark.content,
        })
        .collect::<Vec<_>>();

    let previous_feedback =
        Message::find_for_participant(analysis.user_id, Some(analysis.id), 20, &context.pool)?
            .into_iter()
            .filter(|message| message.message_type == MessageType::MentorFeedback)
            .map(|message| PreviousFeedbackContextItem {
                id: message.id,
                sender_user_id: message.sender_user_id,
                title: message.title,
                content: message.content,
                created_at: message.created_at,
            })
            .collect::<Vec<_>>();

    Ok(MentorFeedbackPromptContext {
        analysis_id: analysis.id,
        analysis_type: analysis.landscape_analysis_type.to_db().to_string(),
        period_start: analysis.period_start,
        period_end: analysis.period_end,
        recipient_user: UserContextItem {
            id: recipient_user.id,
            first_name: recipient_user.first_name,
            last_name: recipient_user.last_name,
            handle: recipient_user.handle,
            biography: recipient_user.biography,
            high_level_projects_definition: recipient_user.high_level_projects_definition,
        },
        current_state_summary: analysis.plain_text_state_summary.clone(),
        period_summary,
        traces,
        landmarks,
        previous_feedback,
    })
}

trait TraceCollectionForAnalysis {
    fn get_input_traces(&self, pool: &crate::db::DbPool) -> Result<Vec<Trace>, PpdcError>;
}

impl TraceCollectionForAnalysis for LandscapeAnalysis {
    fn get_input_traces(&self, pool: &crate::db::DbPool) -> Result<Vec<Trace>, PpdcError> {
        let mut trace_ids = self
            .get_inputs(pool)?
            .into_iter()
            .filter_map(|input| input.trace_id)
            .collect::<Vec<_>>();
        if let Some(trace_id) = self.analyzed_trace_id {
            trace_ids.push(trace_id);
        }
        trace_ids.sort();
        trace_ids.dedup();
        trace_ids
            .into_iter()
            .map(|trace_id| Trace::find_full_trace(trace_id, pool))
            .collect()
    }
}
