use diesel::prelude::*;
use diesel::sql_types::{Bool, Nullable, Text, Timestamp, Uuid as SqlUuid};

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};

use super::model::{NewTrace, Trace, TraceStatus};

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: uuid::Uuid,
}

impl Trace {
    pub fn update(self, pool: &DbPool) -> Result<Trace, PpdcError> {
        if self.is_encrypted && self.encryption_metadata.is_none() {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "encryption_metadata is required when is_encrypted is true".to_string(),
            ));
        }
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let _ = diesel::sql_query(
            "UPDATE traces
             SET title = $2,
                 subtitle = $3,
                 content = $4,
                 is_encrypted = $5,
                 encryption_metadata = CAST($6 AS jsonb),
                 interaction_date = $7,
                 trace_type = $8,
                 status = $9,
                 journal_id = $10,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind::<SqlUuid, _>(self.id)
        .bind::<Text, _>(&self.title)
        .bind::<Text, _>(&self.subtitle)
        .bind::<Text, _>(&self.content)
        .bind::<Bool, _>(self.is_encrypted)
        .bind::<Nullable<Text>, _>(
            self.encryption_metadata
                .as_ref()
                .map(|value| value.to_string()),
        )
        .bind::<Timestamp, _>(self.interaction_date)
        .bind::<Text, _>(self.trace_type.to_db())
        .bind::<Text, _>(self.status.to_db())
        .bind::<Nullable<SqlUuid>, _>(self.journal_id)
        .execute(&mut conn)?;

        Trace::find_full_trace(self.id, pool)
    }

    pub fn set_status(mut self, status: TraceStatus, pool: &DbPool) -> Result<Trace, PpdcError> {
        self.status = status;
        self.update(pool)
    }
}

impl NewTrace {
    pub fn create(self, pool: &DbPool) -> Result<Trace, PpdcError> {
        if self.is_encrypted && self.encryption_metadata.is_none() {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "encryption_metadata is required when is_encrypted is true".to_string(),
            ));
        }
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let inserted = diesel::sql_query(
            "INSERT INTO traces (
                id,
                user_id,
                journal_id,
                title,
                subtitle,
                content,
                is_encrypted,
                encryption_metadata,
                interaction_date,
                trace_type,
                status
             ) VALUES (
                uuid_generate_v4(),
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                CAST($7 AS jsonb),
                $8,
                $9,
                'DRAFT'
             )
             RETURNING id",
        )
        .bind::<SqlUuid, _>(self.user_id)
        .bind::<SqlUuid, _>(self.journal_id)
        .bind::<Text, _>(self.title)
        .bind::<Text, _>(self.subtitle)
        .bind::<Text, _>(self.content)
        .bind::<Bool, _>(self.is_encrypted)
        .bind::<Nullable<Text>, _>(
            self.encryption_metadata
                .as_ref()
                .map(|value| value.to_string()),
        )
        .bind::<Timestamp, _>(self.interaction_date)
        .bind::<Text, _>(self.trace_type.to_db())
        .get_result::<IdRow>(&mut conn)?;

        Trace::find_full_trace(inserted.id, pool)
    }
}
