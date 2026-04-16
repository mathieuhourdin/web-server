use diesel::prelude::*;
use diesel::sql_types::{Bool, Text, Uuid as SqlUuid};

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;

use super::model::{Journal, JournalType, NewJournalDto};

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: uuid::Uuid,
}

impl Journal {
    pub fn update(self, pool: &DbPool) -> Result<Journal, PpdcError> {
        let mut conn = pool.get()?;

        let _ = diesel::sql_query(
            "UPDATE journals
             SET title = $2,
                 subtitle = $3,
                 content = $4,
                 is_encrypted = $5,
                 journal_type = $6,
                 status = $7,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind::<SqlUuid, _>(self.id)
        .bind::<Text, _>(&self.title)
        .bind::<Text, _>(&self.subtitle)
        .bind::<Text, _>(&self.content)
        .bind::<Bool, _>(self.is_encrypted)
        .bind::<Text, _>(self.journal_type.to_db())
        .bind::<Text, _>(self.status.to_db())
        .execute(&mut conn)?;

        Journal::find_full(self.id, pool)
    }

    pub fn create(
        payload: NewJournalDto,
        user_id: uuid::Uuid,
        pool: &DbPool,
    ) -> Result<Journal, PpdcError> {
        let mut conn = pool.get()?;

        let journal_type = payload.journal_type.unwrap_or(JournalType::WorkLogJournal);
        let is_encrypted = payload.is_encrypted.unwrap_or(false);

        let inserted = diesel::sql_query(
            "INSERT INTO journals (id, user_id, title, subtitle, content, is_encrypted, last_trace_at, journal_type, status)
             VALUES (uuid_generate_v4(), $1, $2, $3, $4, $5, NULL, $6, 'DRAFT')
             RETURNING id",
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Text, _>(payload.title)
        .bind::<Text, _>(payload.subtitle.unwrap_or_default())
        .bind::<Text, _>(payload.content.unwrap_or_default())
        .bind::<Bool, _>(is_encrypted)
        .bind::<Text, _>(journal_type.to_db())
        .get_result::<IdRow>(&mut conn)?;

        Journal::find_full(inserted.id, pool)
    }
}
