use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    landscape_analysis::LandscapeAnalysis,
    session::Session,
    trace::Trace,
};

use super::model::{Message, NewMessage, NewMessageDto};

#[derive(Deserialize)]
pub struct MessageFiltersQuery {
    pub landscape_analysis_id: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct AnalysisMessagesQuery {
    pub limit: Option<i64>,
}

#[debug_handler]
pub async fn get_messages_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(filters): Query<MessageFiltersQuery>,
) -> Result<Json<Vec<Message>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let messages = Message::find_for_participant(
        user_id,
        filters.landscape_analysis_id,
        filters.limit.unwrap_or(50),
        &pool,
    )?;
    Ok(Json(messages))
}

#[debug_handler]
pub async fn get_analysis_messages_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(analysis_id): Path<Uuid>,
    Query(params): Query<AnalysisMessagesQuery>,
) -> Result<Json<Vec<Message>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, &pool)?;
    if analysis.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let messages = Message::find_for_participant(user_id, Some(analysis_id), params.limit.unwrap_or(50), &pool)?;
    Ok(Json(messages))
}

#[debug_handler]
pub async fn get_message_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Message>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let message = Message::find(id, &pool)?;
    if message.sender_user_id != user_id && message.recipient_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(message))
}

#[debug_handler]
pub async fn post_message_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewMessageDto>,
) -> Result<Json<Message>, PpdcError> {
    let sender_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;

    if let Some(analysis_id) = payload.landscape_analysis_id {
        let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, &pool)?;
        if analysis.user_id != payload.recipient_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Recipient must own the linked analysis".to_string(),
            ));
        }
    }

    if let Some(trace_id) = payload.trace_id {
        let trace = Trace::find_full_trace(trace_id, &pool)?;
        if trace.user_id != payload.recipient_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Recipient must own the linked trace".to_string(),
            ));
        }
    }

    let message = NewMessage::new(payload, sender_user_id).create(&pool)?;
    Ok(Json(message))
}

#[debug_handler]
pub async fn put_message_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewMessageDto>,
) -> Result<Json<Message>, PpdcError> {
    let sender_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut message = Message::find(id, &pool)?;
    if message.sender_user_id != sender_user_id {
        return Err(PpdcError::unauthorized());
    }

    if let Some(analysis_id) = payload.landscape_analysis_id {
        let analysis = LandscapeAnalysis::find_full_analysis(analysis_id, &pool)?;
        if analysis.user_id != payload.recipient_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Recipient must own the linked analysis".to_string(),
            ));
        }
    }

    if let Some(trace_id) = payload.trace_id {
        let trace = Trace::find_full_trace(trace_id, &pool)?;
        if trace.user_id != payload.recipient_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Recipient must own the linked trace".to_string(),
            ));
        }
    }

    message.recipient_user_id = payload.recipient_user_id;
    message.landscape_analysis_id = payload.landscape_analysis_id;
    message.trace_id = payload.trace_id;
    message.message_type = payload.message_type.unwrap_or(message.message_type);
    message.title = payload.title.unwrap_or_default();
    message.content = payload.content;
    let message = message.update(&pool)?;
    Ok(Json(message))
}
