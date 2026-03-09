use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use chrono::Utc;
use std::cmp::Reverse;
use std::collections::HashSet;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    journal::Journal,
    landscape_analysis::LandscapeAnalysis,
    lens::Lens,
    message::{
        Message, MessageAttachment, MessageAttachmentType, MessageProcessingState, MessageType,
        NewMessage,
    },
    session::Session,
    user::User,
};
use crate::work_analyzer;

use super::{
    llm_qualify,
    model::{NewTrace, NewTraceDto, Trace},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct TraceMessagesQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct NewTraceMessageDto {
    pub title: Option<String>,
    pub content: String,
    pub message_type: Option<MessageType>,
    pub attachment_type: Option<MessageAttachmentType>,
    pub attachment: Option<MessageAttachment>,
}

#[derive(Serialize)]
pub struct TraceMessageCreationResponse {
    pub question_message: Message,
    pub pending_reply_message: Message,
}

#[debug_handler]
pub async fn post_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewTraceDto>,
) -> Result<Json<Trace>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(payload.journal_id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let qualified = llm_qualify::qualify_trace(payload.content.as_str()).await?;
    let request_received_at = Utc::now().naive_utc();

    let interaction_date = qualified
        .interaction_date
        .and_then(|d| d.and_hms_opt(12, 0, 0))
        .unwrap_or(request_received_at);

    let trace = NewTrace::new(
        qualified.title,
        qualified.subtitle,
        payload.content,
        interaction_date,
        user_id,
        journal.id,
    )
    .create(&pool)?;

    let user_lenses = Lens::get_user_lenses(user_id, &pool)?;
    for lens in user_lenses.into_iter().filter(|lens| lens.autoplay) {
        let lens = if lens.target_trace_id != Some(trace.id) {
            lens.update_target_trace(Some(trace.id), &pool)?
        } else {
            lens
        };
        tokio::spawn(async move { work_analyzer::run_lens(lens.id).await });
    }

    Ok(Json(trace))
}

#[debug_handler]
pub async fn get_all_traces_for_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<Trace>>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if session_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let traces = Trace::get_all_for_user(user_id, &pool)?;
    Ok(Json(traces))
}

#[debug_handler]
pub async fn get_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Trace>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != session_user_id {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(trace))
}

#[debug_handler]
pub async fn get_trace_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
) -> Result<Json<LandscapeAnalysis>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let user = User::find(&user_id, &pool)?;
    let current_lens_id = user.current_lens_id.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "current_lens_id is not set for user".to_string(),
        )
    })?;

    let lens = Lens::find_full_lens(current_lens_id, &pool)?;
    if lens.user_id != Some(user_id) {
        return Err(PpdcError::unauthorized());
    }
    let analysis_scope_ids = lens
        .get_analysis_scope_ids(&pool)?
        .into_iter()
        .collect::<HashSet<_>>();

    let mut analyses = LandscapeAnalysis::find_by_trace(trace_id, &pool)?
        .into_iter()
        .filter(|analysis| analysis_scope_ids.contains(&analysis.id))
        .collect::<Vec<_>>();
    analyses
        .sort_by_key(|analysis| Reverse(analysis.interaction_date.unwrap_or(analysis.created_at)));

    let analysis = analyses.into_iter().next().ok_or_else(|| {
        PpdcError::new(
            404,
            ErrorType::ApiError,
            "No analysis found for trace in current lens scope".to_string(),
        )
    })?;
    Ok(Json(analysis))
}

#[debug_handler]
pub async fn get_trace_messages_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
    Query(params): Query<TraceMessagesQuery>,
) -> Result<Json<Vec<Message>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let messages = Message::find_for_trace_conversation(
        user_id,
        trace_id,
        params.limit.unwrap_or(100),
        &pool,
    )?;
    Ok(Json(messages))
}

#[debug_handler]
pub async fn post_trace_message_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
    Json(payload): Json<NewTraceMessageDto>,
) -> Result<Json<TraceMessageCreationResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let user = User::find(&user_id, &pool)?;
    let mentor_id = user.mentor_id.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "No mentor assigned to user".to_string(),
        )
    })?;
    let question_message_type = payload.message_type.unwrap_or(MessageType::Question);
    if !matches!(
        question_message_type,
        MessageType::Question | MessageType::TarotReadingRequest
    ) {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "trace message message_type must be question or tarot_reading_request".to_string(),
        ));
    }
    if question_message_type == MessageType::TarotReadingRequest {
        let has_tarot_attachment = matches!(
            (payload.attachment_type, payload.attachment.as_ref()),
            (
                Some(MessageAttachmentType::TarotReading),
                Some(MessageAttachment::TarotReading(_))
            )
        );
        if !has_tarot_attachment {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "tarot_reading_request requires attachment_type=tarot_reading and a tarot attachment payload".to_string(),
            ));
        }
    }

    let question_message = NewMessage {
        sender_user_id: user_id,
        recipient_user_id: mentor_id,
        landscape_analysis_id: None,
        trace_id: Some(trace_id),
        reply_to_message_id: None,
        message_type: question_message_type,
        processing_state: MessageProcessingState::Processed,
        title: payload.title.unwrap_or_default(),
        content: payload.content,
        attachment_type: payload.attachment_type,
        attachment: payload.attachment,
    }
    .create(&pool)?;

    let pending_reply_message = NewMessage {
        sender_user_id: mentor_id,
        recipient_user_id: user_id,
        landscape_analysis_id: None,
        trace_id: Some(trace_id),
        reply_to_message_id: Some(question_message.id),
        message_type: MessageType::MentorReply,
        processing_state: MessageProcessingState::Pending,
        title: String::new(),
        content: String::new(),
        attachment_type: None,
        attachment: None,
    }
    .create(&pool)?;

    let pool_for_task = pool.clone();
    let pending_reply_id = pending_reply_message.id;
    tokio::spawn(async move {
        let _ = work_analyzer::run_message(pending_reply_id, &pool_for_task).await;
    });

    Ok(Json(TraceMessageCreationResponse {
        question_message,
        pending_reply_message,
    }))
}

#[debug_handler]
pub async fn get_traces_for_journal_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Trace>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let traces = Trace::get_all_for_journal(id, &pool)?;
    Ok(Json(traces))
}
