use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types::{Text, Timestamp, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;

use super::model::{Journal, JournalStatus, JournalType};

#[derive(QueryableByName)]
struct JournalRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    subtitle: String,
    #[diesel(sql_type = Text)]
    content: String,
    #[diesel(sql_type = Text)]
    journal_type: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
}

impl From<JournalRow> for Journal {
    fn from(row: JournalRow) -> Self {
        Journal {
            id: row.id,
            title: row.title,
            subtitle: row.subtitle,
            content: row.content,
            user_id: row.user_id,
            status: JournalStatus::from_db(&row.status),
            journal_type: JournalType::from_db(&row.journal_type),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl Journal {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Journal, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let row = diesel::sql_query(
            "SELECT id, user_id, title, subtitle, content, journal_type, status, created_at, updated_at
             FROM journals
             WHERE id = $1",
        )
        .bind::<SqlUuid, _>(id)
        .get_result::<JournalRow>(&mut conn)?;

        Ok(row.into())
    }

    pub fn find_full(id: Uuid, pool: &DbPool) -> Result<Journal, PpdcError> {
        Journal::find(id, pool)
    }

    pub fn find_for_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<Journal>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let rows = diesel::sql_query(
            "SELECT id, user_id, title, subtitle, content, journal_type, status, created_at, updated_at
             FROM journals
             WHERE user_id = $1
             ORDER BY updated_at DESC",
        )
        .bind::<SqlUuid, _>(user_id)
        .load::<JournalRow>(&mut conn)?;

        Ok(rows.into_iter().map(Journal::from).collect())
    }
}
