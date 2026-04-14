use chrono::NaiveDateTime;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::{Nullable, Text};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::messages;

use super::attachment::{MessageAttachment, MessageAttachmentType};
use super::model::{Message, MessageMetadata, MessageProcessingState, MessageType};

type MessageTuple = (
    Uuid,
    Uuid,
    Uuid,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<NaiveDateTime>,
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
        post_id,
        reply_to_message_id,
        message_type_raw,
        processing_state_raw,
        title,
        content,
        attachment_type_raw,
        attachment_json,
        metadata_json,
        seen_at,
        created_at,
        updated_at,
    ) = row;

    let attachment_type = attachment_type_raw
        .as_deref()
        .and_then(MessageAttachmentType::from_db);
    let attachment = match (attachment_type, attachment_json) {
        (Some(kind), Some(json)) => MessageAttachment::from_json_string(kind, &json),
        _ => None,
    };
    let metadata = metadata_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<MessageMetadata>(json).ok());

    Message {
        id,
        sender_user_id,
        recipient_user_id,
        landscape_analysis_id,
        trace_id,
        post_id,
        reply_to_message_id,
        message_type: MessageType::from_db(&message_type_raw),
        processing_state: MessageProcessingState::from_db(&processing_state_raw),
        title,
        content,
        attachment_type,
        attachment,
        metadata,
        seen_at,
        created_at,
        updated_at,
    }
}

impl Message {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Message, PpdcError> {
        let mut conn = pool
            .get()?;
        let row = messages::table
            .filter(messages::id.eq(id))
            .select((
                messages::id,
                messages::sender_user_id,
                messages::recipient_user_id,
                messages::landscape_analysis_id,
                messages::trace_id,
                messages::post_id,
                messages::reply_to_message_id,
                messages::message_type,
                messages::processing_state,
                messages::title,
                messages::content,
                messages::attachment_type,
                sql::<Nullable<Text>>("attachment::text"),
                sql::<Nullable<Text>>("metadata::text"),
                messages::seen_at,
                messages::created_at,
                messages::updated_at,
            ))
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
        let (items, _) =
            Self::find_for_participant_paginated(user_id, landscape_analysis_id, 0, limit, pool)?;
        Ok(items)
    }

    pub fn find_for_participant_paginated(
        user_id: Uuid,
        landscape_analysis_id: Option<Uuid>,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Message>, i64), PpdcError> {
        Self::find_for_participant_filtered_paginated(
            user_id,
            landscape_analysis_id,
            false,
            false,
            offset,
            limit,
            pool,
        )
    }

    pub fn find_for_participant_filtered_paginated(
        user_id: Uuid,
        landscape_analysis_id: Option<Uuid>,
        received_only: bool,
        unread_only: bool,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Message>, i64), PpdcError> {
        let mut conn = pool
            .get()?;
        let mut count_query = messages::table.into_boxed();

        if received_only || unread_only {
            count_query = count_query.filter(messages::recipient_user_id.eq(user_id));
        } else {
            count_query = count_query.filter(
                messages::sender_user_id
                    .eq(user_id)
                    .or(messages::recipient_user_id.eq(user_id)),
            );
        }

        if unread_only {
            count_query = count_query.filter(messages::seen_at.is_null());
        }

        if let Some(landscape_analysis_id) = landscape_analysis_id {
            count_query =
                count_query.filter(messages::landscape_analysis_id.eq(Some(landscape_analysis_id)));
        }

        let total = count_query.count().get_result::<i64>(&mut conn)?;

        let mut query = messages::table.into_boxed();

        if received_only || unread_only {
            query = query.filter(messages::recipient_user_id.eq(user_id));
        } else {
            query = query.filter(
                messages::sender_user_id
                    .eq(user_id)
                    .or(messages::recipient_user_id.eq(user_id)),
            );
        }

        if unread_only {
            query = query.filter(messages::seen_at.is_null());
        }

        if let Some(landscape_analysis_id) = landscape_analysis_id {
            query = query.filter(messages::landscape_analysis_id.eq(Some(landscape_analysis_id)));
        }

        let rows = query
            .select((
                messages::id,
                messages::sender_user_id,
                messages::recipient_user_id,
                messages::landscape_analysis_id,
                messages::trace_id,
                messages::post_id,
                messages::reply_to_message_id,
                messages::message_type,
                messages::processing_state,
                messages::title,
                messages::content,
                messages::attachment_type,
                sql::<Nullable<Text>>("attachment::text"),
                sql::<Nullable<Text>>("metadata::text"),
                messages::seen_at,
                messages::created_at,
                messages::updated_at,
            ))
            .order(messages::created_at.desc())
            .offset(offset)
            .limit(limit.max(1))
            .load::<MessageTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_message).collect(), total))
    }

    pub fn find_for_trace_conversation(
        user_id: Uuid,
        trace_id: Uuid,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Message>, PpdcError> {
        let (items, _) =
            Self::find_for_trace_conversation_paginated(user_id, trace_id, 0, limit, pool)?;
        Ok(items)
    }

    pub fn find_for_trace_conversation_paginated(
        user_id: Uuid,
        trace_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Message>, i64), PpdcError> {
        let mut conn = pool
            .get()?;
        let total = messages::table
            .filter(messages::trace_id.eq(Some(trace_id)))
            .filter(
                messages::sender_user_id
                    .eq(user_id)
                    .or(messages::recipient_user_id.eq(user_id)),
            )
            .count()
            .get_result::<i64>(&mut conn)?;

        let rows = messages::table
            .filter(messages::trace_id.eq(Some(trace_id)))
            .filter(
                messages::sender_user_id
                    .eq(user_id)
                    .or(messages::recipient_user_id.eq(user_id)),
            )
            .select((
                messages::id,
                messages::sender_user_id,
                messages::recipient_user_id,
                messages::landscape_analysis_id,
                messages::trace_id,
                messages::post_id,
                messages::reply_to_message_id,
                messages::message_type,
                messages::processing_state,
                messages::title,
                messages::content,
                messages::attachment_type,
                sql::<Nullable<Text>>("attachment::text"),
                sql::<Nullable<Text>>("metadata::text"),
                messages::seen_at,
                messages::created_at,
                messages::updated_at,
            ))
            .order(messages::created_at.desc())
            .offset(offset)
            .limit(limit.max(1))
            .load::<MessageTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_message).collect(), total))
    }

    pub fn find_for_post_conversation_paginated(
        user_id: Uuid,
        post_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Message>, i64), PpdcError> {
        let mut conn = pool
            .get()?;
        let total = messages::table
            .filter(messages::post_id.eq(Some(post_id)))
            .filter(
                messages::sender_user_id
                    .eq(user_id)
                    .or(messages::recipient_user_id.eq(user_id)),
            )
            .count()
            .get_result::<i64>(&mut conn)?;

        let rows = messages::table
            .filter(messages::post_id.eq(Some(post_id)))
            .filter(
                messages::sender_user_id
                    .eq(user_id)
                    .or(messages::recipient_user_id.eq(user_id)),
            )
            .select((
                messages::id,
                messages::sender_user_id,
                messages::recipient_user_id,
                messages::landscape_analysis_id,
                messages::trace_id,
                messages::post_id,
                messages::reply_to_message_id,
                messages::message_type,
                messages::processing_state,
                messages::title,
                messages::content,
                messages::attachment_type,
                sql::<Nullable<Text>>("attachment::text"),
                sql::<Nullable<Text>>("metadata::text"),
                messages::seen_at,
                messages::created_at,
                messages::updated_at,
            ))
            .order(messages::created_at.desc())
            .offset(offset)
            .limit(limit.max(1))
            .load::<MessageTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_message).collect(), total))
    }

    pub fn find_latest_feedback_for_analysis(
        analysis_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Message>, PpdcError> {
        let mut conn = pool
            .get()?;
        let row = messages::table
            .filter(messages::landscape_analysis_id.eq(Some(analysis_id)))
            .filter(messages::recipient_user_id.eq(user_id))
            .filter(messages::sender_user_id.ne(user_id))
            .filter(messages::message_type.eq(MessageType::MentorFeedback.to_db()))
            .filter(messages::content.ne(""))
            .select((
                messages::id,
                messages::sender_user_id,
                messages::recipient_user_id,
                messages::landscape_analysis_id,
                messages::trace_id,
                messages::post_id,
                messages::reply_to_message_id,
                messages::message_type,
                messages::processing_state,
                messages::title,
                messages::content,
                messages::attachment_type,
                sql::<Nullable<Text>>("attachment::text"),
                sql::<Nullable<Text>>("metadata::text"),
                messages::seen_at,
                messages::created_at,
                messages::updated_at,
            ))
            .order(messages::created_at.desc())
            .first::<MessageTuple>(&mut conn)
            .optional()?;
        Ok(row.map(tuple_to_message))
    }

    pub fn find_recent_mentor_feedbacks_for_user(
        user_id: Uuid,
        limit: i64,
        pool: &DbPool,
    ) -> Result<Vec<Message>, PpdcError> {
        let mut conn = pool
            .get()?;
        let rows = messages::table
            .filter(messages::recipient_user_id.eq(user_id))
            .filter(messages::sender_user_id.ne(user_id))
            .filter(messages::message_type.eq(MessageType::MentorFeedback.to_db()))
            .filter(messages::content.ne(""))
            .select((
                messages::id,
                messages::sender_user_id,
                messages::recipient_user_id,
                messages::landscape_analysis_id,
                messages::trace_id,
                messages::post_id,
                messages::reply_to_message_id,
                messages::message_type,
                messages::processing_state,
                messages::title,
                messages::content,
                messages::attachment_type,
                sql::<Nullable<Text>>("attachment::text"),
                sql::<Nullable<Text>>("metadata::text"),
                messages::seen_at,
                messages::created_at,
                messages::updated_at,
            ))
            .order(messages::created_at.desc())
            .limit(limit.max(1))
            .load::<MessageTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_message).collect())
    }
}
