use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::schema::journals;

use super::model::{Journal, JournalStatus, JournalType};

type JournalTuple = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

impl From<JournalTuple> for Journal {
    fn from(row: JournalTuple) -> Self {
        let (id, user_id, title, subtitle, content, journal_type, status, created_at, updated_at) = row;
        Journal {
            id,
            title,
            subtitle,
            content,
            user_id,
            status: JournalStatus::from_db(&status),
            journal_type: JournalType::from_db(&journal_type),
            created_at,
            updated_at,
        }
    }
}

impl Journal {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Journal, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let row = journals::table
            .filter(journals::id.eq(id))
            .select((
                journals::id,
                journals::user_id,
                journals::title,
                journals::subtitle,
                journals::content,
                journals::journal_type,
                journals::status,
                journals::created_at,
                journals::updated_at,
            ))
            .first::<JournalTuple>(&mut conn)?;

        Ok(row.into())
    }

    pub fn find_full(id: Uuid, pool: &DbPool) -> Result<Journal, PpdcError> {
        Journal::find(id, pool)
    }

    pub fn find_for_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<Journal>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let rows = journals::table
            .filter(journals::user_id.eq(user_id))
            .select((
                journals::id,
                journals::user_id,
                journals::title,
                journals::subtitle,
                journals::content,
                journals::journal_type,
                journals::status,
                journals::created_at,
                journals::updated_at,
            ))
            .order(journals::updated_at.desc())
            .load::<JournalTuple>(&mut conn)?;

        Ok(rows.into_iter().map(Journal::from).collect())
    }
}
