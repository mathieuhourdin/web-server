use diesel::prelude::*;
use diesel::sql_types::{Bool, Text, Uuid as SqlUuid};

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::trace::Trace;

use super::model::{Journal, JournalSharingMode, JournalStatus, JournalType, NewJournalDto};

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
                 sharing_mode = $7,
                 status = $8,
                 current_draft_id = $9,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind::<SqlUuid, _>(self.id)
        .bind::<Text, _>(&self.title)
        .bind::<Text, _>(&self.subtitle)
        .bind::<Text, _>(&self.content)
        .bind::<Bool, _>(self.is_encrypted)
        .bind::<Text, _>(self.journal_type.to_db())
        .bind::<Text, _>(self.sharing_mode.to_db())
        .bind::<Text, _>(self.status.to_db())
        .bind::<diesel::sql_types::Nullable<SqlUuid>, _>(self.current_draft_id)
        .execute(&mut conn)?;

        Journal::find_full(self.id, pool)
    }

    pub fn create(
        payload: NewJournalDto,
        user_id: uuid::Uuid,
        pool: &DbPool,
    ) -> Result<Journal, PpdcError> {
        let mut conn = pool.get()?;

        let journal_type = payload.journal_type.unwrap_or(JournalType::UserJournal);
        let sharing_mode = payload.sharing_mode.unwrap_or(JournalSharingMode::Shared);
        let is_encrypted = payload.is_encrypted.unwrap_or(false);
        let status = JournalStatus::Active;

        let inserted = diesel::sql_query(
            "INSERT INTO journals (id, user_id, title, subtitle, content, is_encrypted, last_trace_at, journal_type, sharing_mode, status, current_draft_id)
             VALUES (uuid_generate_v4(), $1, $2, $3, $4, $5, NULL, $6, $7, $8, NULL)
             RETURNING id",
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Text, _>(payload.title)
        .bind::<Text, _>(payload.subtitle.unwrap_or_default())
        .bind::<Text, _>(payload.content.unwrap_or_default())
        .bind::<Bool, _>(is_encrypted)
        .bind::<Text, _>(journal_type.to_db())
        .bind::<Text, _>(sharing_mode.to_db())
        .bind::<Text, _>(status.to_db())
        .get_result::<IdRow>(&mut conn)?;

        let journal = Journal::find_full(inserted.id, pool)?;
        if journal.status == JournalStatus::Active
            && journal.journal_type == JournalType::UserJournal
        {
            // The journal row must exist before its persisted draft trace can be created.
            // This invariant is therefore completed in application logic immediately after
            // insert, rather than by an insert-time SQL CHECK on journals.current_draft_id.
            let draft = Trace::create_blank_draft_for_journal(&journal, pool)?;
            let mut journal = journal;
            journal.current_draft_id = Some(draft.id);
            return journal.update(pool);
        }

        Journal::find_full(inserted.id, pool)
    }
}
