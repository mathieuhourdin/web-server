use std::collections::HashMap;

use chrono::NaiveDateTime;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Array, BigInt, Bool, Text, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::{message_reactions, messages};

use super::model::Message;

#[derive(Debug, Clone, Queryable, Selectable, Serialize)]
#[diesel(table_name = message_reactions)]
pub struct MessageReaction {
    pub id: Uuid,
    pub message_id: Uuid,
    pub user_id: Uuid,
    pub emoji: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct MessageReactionSummary {
    pub emoji: String,
    pub count: i64,
    pub reacted_by_me: bool,
}

#[derive(QueryableByName)]
struct ReactionSummaryRow {
    #[diesel(sql_type = SqlUuid)]
    message_id: Uuid,
    #[diesel(sql_type = Text)]
    emoji: String,
    #[diesel(sql_type = BigInt)]
    count: i64,
    #[diesel(sql_type = Bool)]
    reacted_by_me: bool,
}

impl MessageReaction {
    pub const MAX_EMOJI_CHARACTERS: usize = 16;

    pub fn normalize_emoji(emoji: &str) -> Result<String, PpdcError> {
        let emoji = emoji.trim();
        let length = emoji.chars().count();
        if length == 0 || length > Self::MAX_EMOJI_CHARACTERS {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!(
                    "emoji must contain between 1 and {} characters",
                    Self::MAX_EMOJI_CHARACTERS
                ),
            ));
        }
        Ok(emoji.to_string())
    }

    pub fn upsert(
        message_id: Uuid,
        user_id: Uuid,
        emoji: &str,
        pool: &DbPool,
    ) -> Result<(Self, bool), PpdcError> {
        let emoji = Self::normalize_emoji(emoji)?;
        let mut conn = pool.get()?;
        conn.transaction::<_, PpdcError, _>(|conn| {
            messages::table
                .filter(messages::id.eq(message_id))
                .select(messages::id)
                .for_update()
                .first::<Uuid>(conn)?;
            let existing = message_reactions::table
                .filter(message_reactions::message_id.eq(message_id))
                .filter(message_reactions::user_id.eq(user_id))
                .select(Self::as_select())
                .for_update()
                .first::<Self>(conn)
                .optional()?;
            if let Some(existing) = existing {
                if existing.emoji == emoji {
                    return Ok((existing, false));
                }
                let updated = diesel::update(
                    message_reactions::table.filter(message_reactions::id.eq(existing.id)),
                )
                .set((
                    message_reactions::emoji.eq(emoji),
                    message_reactions::updated_at.eq(diesel::dsl::now),
                ))
                .returning(Self::as_returning())
                .get_result::<Self>(conn)?;
                return Ok((updated, true));
            }
            let created = diesel::insert_into(message_reactions::table)
                .values((
                    message_reactions::id.eq(Uuid::new_v4()),
                    message_reactions::message_id.eq(message_id),
                    message_reactions::user_id.eq(user_id),
                    message_reactions::emoji.eq(emoji),
                ))
                .returning(Self::as_returning())
                .get_result::<Self>(conn)?;
            Ok((created, true))
        })
    }

    pub fn delete_for_user(
        message_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let mut conn = pool.get()?;
        conn.transaction::<_, PpdcError, _>(|conn| {
            messages::table
                .filter(messages::id.eq(message_id))
                .select(messages::id)
                .for_update()
                .first::<Uuid>(conn)?;
            Ok(diesel::delete(
                message_reactions::table
                    .filter(message_reactions::message_id.eq(message_id))
                    .filter(message_reactions::user_id.eq(user_id)),
            )
            .execute(conn)?
                > 0)
        })
    }

    pub fn hydrate_messages(
        viewer_user_id: Uuid,
        messages: Vec<Message>,
        pool: &DbPool,
    ) -> Result<Vec<Message>, PpdcError> {
        let mut conn = pool.get()?;
        Self::hydrate_messages_with_conn(viewer_user_id, messages, &mut conn)
    }

    pub(crate) fn hydrate_messages_with_conn(
        viewer_user_id: Uuid,
        mut messages: Vec<Message>,
        conn: &mut PgConnection,
    ) -> Result<Vec<Message>, PpdcError> {
        if messages.is_empty() {
            return Ok(messages);
        }
        let message_ids = messages
            .iter()
            .map(|message| message.id)
            .collect::<Vec<_>>();
        let rows = sql_query(
            r#"
            SELECT
                message_id,
                emoji,
                COUNT(*)::bigint AS count,
                BOOL_OR(user_id = $2) AS reacted_by_me
            FROM message_reactions
            WHERE message_id = ANY($1)
            GROUP BY message_id, emoji
            ORDER BY message_id, MIN(created_at), emoji
            "#,
        )
        .bind::<Array<SqlUuid>, _>(&message_ids)
        .bind::<SqlUuid, _>(viewer_user_id)
        .load::<ReactionSummaryRow>(conn)?;

        let mut by_message_id = HashMap::<Uuid, Vec<MessageReactionSummary>>::new();
        for row in rows {
            by_message_id
                .entry(row.message_id)
                .or_default()
                .push(MessageReactionSummary {
                    emoji: row.emoji,
                    count: row.count,
                    reacted_by_me: row.reacted_by_me,
                });
        }
        for message in &mut messages {
            message.reactions = by_message_id.remove(&message.id).unwrap_or_default();
        }
        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_reaction_length() {
        assert_eq!(MessageReaction::normalize_emoji(" ❤️ ").unwrap(), "❤️");
        assert!(MessageReaction::normalize_emoji("").is_err());
        assert!(MessageReaction::normalize_emoji(&"x".repeat(17)).is_err());
    }
}
