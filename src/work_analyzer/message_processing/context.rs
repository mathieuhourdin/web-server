use serde::Serialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::landmark::LandmarkType;
use crate::entities_v2::landscape_analysis::LandscapeAnalysis;
use crate::entities_v2::lens::Lens;
use crate::entities_v2::message::Message;
use crate::entities_v2::trace::Trace;
use crate::entities_v2::user::User;

#[derive(Debug, Serialize)]
pub struct TraceMessageReplyPromptContext {
    pub mentor_name: String,
    pub mentor_biography: Option<String>,
    pub user_question: MessageContextItem,
    pub target_trace: TraceContextItem,
    pub current_user_high_level_projects: Vec<HighLevelProjectContextItem>,
    pub previous_messages_for_trace: Vec<MessageContextItem>,
    pub recent_user_traces: Vec<TraceContextItem>,
}

#[derive(Debug, Serialize)]
pub struct MessageContextItem {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct TraceContextItem {
    pub id: Uuid,
    pub interaction_date: chrono::NaiveDateTime,
    pub trace_type: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct HighLevelProjectContextItem {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
}

pub fn build(
    reply_message: &Message,
    question_message: &Message,
    trace: &Trace,
    mentor_user: &User,
    recipient_user: &User,
    pool: &DbPool,
) -> Result<TraceMessageReplyPromptContext, PpdcError> {
    let previous_messages_for_trace = Message::find_for_trace_conversation(recipient_user.id, trace.id, 50, pool)?
        .into_iter()
        .filter(|message| message.id != reply_message.id && message.id != question_message.id)
        .map(|message| MessageContextItem {
            id: message.id,
            title: message.title,
            content: message.content,
            created_at: message.created_at,
        })
        .collect::<Vec<_>>();

    let mut recent_user_traces = Trace::get_before(recipient_user.id, trace.id, pool)?
        .into_iter()
        .filter(|candidate| candidate.id != trace.id)
        .rev()
        .take(5)
        .collect::<Vec<_>>();
    recent_user_traces.reverse();
    let current_user_high_level_projects = find_current_lens_high_level_projects(recipient_user, pool)?;

    Ok(TraceMessageReplyPromptContext {
        mentor_name: format!("{} {}", mentor_user.first_name, mentor_user.last_name),
        mentor_biography: mentor_user.biography.clone(),
        user_question: MessageContextItem {
            id: question_message.id,
            title: question_message.title.clone(),
            content: question_message.content.clone(),
            created_at: question_message.created_at,
        },
        target_trace: TraceContextItem {
            id: trace.id,
            interaction_date: trace.interaction_date,
            trace_type: trace.trace_type.to_db().to_string(),
            title: trace.title.clone(),
            subtitle: trace.subtitle.clone(),
            content: trace.content.clone(),
        },
        current_user_high_level_projects,
        previous_messages_for_trace,
        recent_user_traces: recent_user_traces
            .into_iter()
            .map(|trace| TraceContextItem {
                id: trace.id,
                interaction_date: trace.interaction_date,
                trace_type: trace.trace_type.to_db().to_string(),
                title: trace.title,
                subtitle: trace.subtitle,
                content: trace.content,
            })
            .collect(),
    })
}

fn find_current_lens_high_level_projects(
    recipient_user: &User,
    pool: &DbPool,
) -> Result<Vec<HighLevelProjectContextItem>, PpdcError> {
    let Some(current_lens_id) = recipient_user.current_lens_id else {
        return Ok(vec![]);
    };
    let lens = Lens::find_full_lens(current_lens_id, pool)?;
    let Some(current_landscape_id) = lens.current_landscape_id else {
        return Ok(vec![]);
    };
    let current_landscape = LandscapeAnalysis::find_full_analysis(current_landscape_id, pool)?;
    let hlps = current_landscape
        .get_landmarks(None, pool)?
        .into_iter()
        .filter(|landmark| landmark.landmark_type == LandmarkType::HighLevelProject)
        .map(|landmark| HighLevelProjectContextItem {
            id: landmark.id,
            title: landmark.title,
            subtitle: landmark.subtitle,
            content: landmark.content,
        })
        .collect::<Vec<_>>();
    Ok(hlps)
}
