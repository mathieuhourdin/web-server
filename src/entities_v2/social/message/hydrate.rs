use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::messages;

use super::model::{Message, MessageProcessingState, MessageType};

type MessageTuple = (
    Uuid,
    Uuid,
    Uuid,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    String,
    String,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

fn tuple_to_message(row: MessageTuple) -> Message {
    let (
        id,
        sender_user_id,
        recipient_user_id,
        landscape_analysis_id,
        trace_id,
        reply_to_message_id,
        message_type_raw,
        processing_state_raw,
        title,
        content,
        created_at,
        updated_at,
    ) = row;

    Message {
        id,
        sender_user_id,
        recipient_user_id,
        landscape_analysis_id,
        trace_id,
        reply_to_message_id,
        message_type: MessageType::from_db(&message_type_raw),
        processing_state: MessageProcessingState::from_db(&processing_state_raw),
        title,
        content,
        created_at,
        updated_at,
    }
}

fn select_message_columns() -> (
    messages::id,
    messages::sender_user_id,
    messages::recipient_user_id,
    messages::landscape_analysis_id,
    messages::trace_id,
    messages::reply_to_message_id,
    messages::message_type,
    messages::processing_state,
    messages::title,
    messages::content,
    messages::created_at,
    messages::updated_at,
) {
    (
        messages::id,
        messages::sender_user_id,
        messages::recipient_user_id,
        messages::landscape_analysis_id,
        messages::trace_id,
        messages::reply_to_message_id,
        messages::message_type,
        messages::processing_state,
        messages::title,
        messages::content,
        messages::created_at,
        messages::updated_at,
    )
}

impl Message {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Message, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let row = messages::table
            .filter(messages::id.eq(id))
            .select(select_message_columns())
            .first::<MessageTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_message).ok_or_else(|| {
            PpdcError::new(404, ErrorType::ApiError, "Message not found".to_string())
        })
    }

    pub fn find_for_participant(
        user_id: Uuid,
        landscape_analysis_id: Option<Uuid>,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Message>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let mut query = messages::table
            .filter(
                messages::sender_user_id
                    .eq(user_id)
                    .or(messages::recipient_user_id.eq(user_id)),
            )
            .into_boxed();

        if let Some(landscape_analysis_id) = landscape_analysis_id {
            query = query.filter(messages::landscape_analysis_id.eq(Some(landscape_analysis_id)));
        }

        let rows = query
            .select(select_message_columns())
            .order(messages::created_at.desc())
            .limit(limit.max(1))
            .load::<MessageTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_message).collect())
    }

    pub fn find_for_trace_conversation(
        user_id: Uuid,
        trace_id: Uuid,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Message>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = messages::table
            .filter(messages::trace_id.eq(Some(trace_id)))
            .filter(
                messages::sender_user_id
                    .eq(user_id)
                    .or(messages::recipient_user_id.eq(user_id)),
            )
            .select(select_message_columns())
            .order(messages::created_at.asc())
            .limit(limit.max(1))
            .load::<MessageTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_message).collect())
    }
}
