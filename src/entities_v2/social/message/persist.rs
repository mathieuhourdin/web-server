use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text, Timestamp, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};

use super::attachment::MessageAttachment;
use super::model::{Message, NewMessage};

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

fn serialize_attachment(
    attachment_type: Option<super::attachment::MessageAttachmentType>,
    attachment: Option<&MessageAttachment>,
) -> Result<(Option<String>, Option<String>), PpdcError> {
    match (attachment_type, attachment) {
        (None, None) => Ok((None, None)),
        (Some(_), None) | (None, Some(_)) => Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "attachment_type and attachment must both be provided or both be null".to_string(),
        )),
        (Some(kind), Some(value)) => {
            if value.attachment_type() != kind {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "attachment does not match attachment_type".to_string(),
                ));
            }
            let attachment_json = value.to_json_string().map_err(|err| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    format!("Invalid attachment payload: {}", err),
                )
            })?;
            Ok((Some(kind.to_db().to_string()), Some(attachment_json)))
        }
    }
}

impl Message {
    pub fn update(self, pool: &DbPool) -> Result<Message, PpdcError> {
        let mut conn = pool
            .get()?;
        let (attachment_type_db, attachment_json) =
            serialize_attachment(self.attachment_type, self.attachment.as_ref())?;

        sql_query(
            r#"
            UPDATE messages
            SET recipient_user_id = $1,
                landscape_analysis_id = $2,
                trace_id = $3,
                post_id = $4,
                reply_to_message_id = $5,
                message_type = $6,
                processing_state = $7,
                title = $8,
                content = $9,
                attachment_type = $10,
                attachment = CAST($11 AS jsonb),
                seen_at = $12,
                updated_at = NOW()
            WHERE id = $13
            "#,
        )
        .bind::<SqlUuid, _>(self.recipient_user_id)
        .bind::<Nullable<SqlUuid>, _>(self.landscape_analysis_id)
        .bind::<Nullable<SqlUuid>, _>(self.trace_id)
        .bind::<Nullable<SqlUuid>, _>(self.post_id)
        .bind::<Nullable<SqlUuid>, _>(self.reply_to_message_id)
        .bind::<Text, _>(self.message_type.to_db())
        .bind::<Text, _>(self.processing_state.to_db())
        .bind::<Text, _>(self.title)
        .bind::<Text, _>(self.content)
        .bind::<Nullable<Text>, _>(attachment_type_db)
        .bind::<Nullable<Text>, _>(attachment_json)
        .bind::<Nullable<Timestamp>, _>(self.seen_at)
        .bind::<SqlUuid, _>(self.id)
        .execute(&mut conn)?;
        Message::find(self.id, pool)
    }

    pub fn mark_seen(self, viewer_user_id: Uuid, pool: &DbPool) -> Result<Message, PpdcError> {
        if self.recipient_user_id != viewer_user_id {
            return Err(PpdcError::unauthorized());
        }
        if self.seen_at.is_some() {
            return Ok(self);
        }

        let mut conn = pool
            .get()?;
        sql_query(
            r#"
            UPDATE messages
            SET seen_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind::<SqlUuid, _>(self.id)
        .execute(&mut conn)?;
        Message::find(self.id, pool)
    }
}

impl NewMessage {
    pub fn create(self, pool: &DbPool) -> Result<Message, PpdcError> {
        let mut conn = pool
            .get()?;
        let (attachment_type_db, attachment_json) =
            serialize_attachment(self.attachment_type, self.attachment.as_ref())?;

        let id = sql_query(
            r#"
            INSERT INTO messages (
                sender_user_id,
                recipient_user_id,
                landscape_analysis_id,
                trace_id,
                post_id,
                reply_to_message_id,
                message_type,
                processing_state,
                title,
                content,
                attachment_type,
                attachment,
                seen_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, CAST($12 AS jsonb), NULL
            )
            RETURNING id
            "#,
        )
        .bind::<SqlUuid, _>(self.sender_user_id)
        .bind::<SqlUuid, _>(self.recipient_user_id)
        .bind::<Nullable<SqlUuid>, _>(self.landscape_analysis_id)
        .bind::<Nullable<SqlUuid>, _>(self.trace_id)
        .bind::<Nullable<SqlUuid>, _>(self.post_id)
        .bind::<Nullable<SqlUuid>, _>(self.reply_to_message_id)
        .bind::<Text, _>(self.message_type.to_db())
        .bind::<Text, _>(self.processing_state.to_db())
        .bind::<Text, _>(self.title)
        .bind::<Text, _>(self.content)
        .bind::<Nullable<Text>, _>(attachment_type_db)
        .bind::<Nullable<Text>, _>(attachment_json)
        .get_result::<IdRow>(&mut conn)?
        .id;
        Message::find(id, pool)
    }
}
