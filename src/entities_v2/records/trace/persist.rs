use chrono::Utc;
use diesel::prelude::*;
use diesel::sql_types::{Bool, Nullable, Text, Timestamp, Timestamptz, Uuid as SqlUuid};

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};

use super::model::{NewTrace, Trace, TraceStatus};

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: uuid::Uuid,
}

fn recalculate_journal_last_trace_at(
    conn: &mut diesel::PgConnection,
    journal_id: uuid::Uuid,
) -> Result<(), diesel::result::Error> {
    diesel::sql_query(
        "UPDATE journals
         SET last_trace_at = (
             SELECT MAX(interaction_date)
             FROM traces
             WHERE journal_id = $1
         ),
         updated_at = NOW()
         WHERE id = $1",
    )
    .bind::<SqlUuid, _>(journal_id)
    .execute(conn)?;
    Ok(())
}

fn sync_trace_image_asset_to_related_posts(
    conn: &mut diesel::PgConnection,
    trace_id: uuid::Uuid,
    image_asset_id: Option<uuid::Uuid>,
) -> Result<(), diesel::result::Error> {
    diesel::sql_query(
        "UPDATE posts
         SET image_asset_id = $2,
             updated_at = NOW()
         WHERE source_trace_id = $1",
    )
    .bind::<SqlUuid, _>(trace_id)
    .bind::<Nullable<SqlUuid>, _>(image_asset_id)
    .execute(conn)?;
    Ok(())
}

impl Trace {
    pub fn update(mut self, pool: &DbPool) -> Result<Trace, PpdcError> {
        if self.is_encrypted && self.encryption_metadata.is_none() {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "encryption_metadata is required when is_encrypted is true".to_string(),
            ));
        }
        if self.timeout_at.is_some() && self.timeout_start_at.is_none() {
            self.timeout_start_at = Some(Utc::now());
        }
        let mut conn = pool
            .get()?;

        conn.transaction::<(), diesel::result::Error, _>(|conn| {
            diesel::sql_query(
                "UPDATE traces
                 SET title = $2,
                     subtitle = $3,
                     content = $4,
                     is_encrypted = $5,
                     encryption_metadata = CAST($6 AS jsonb),
                     image_asset_id = $7,
                     timeout_start_at = $8,
                     timeout_at = $9,
                     interaction_date = $10,
                     trace_type = $11,
                     status = $12,
                     journal_id = $13,
                     start_writing_at = $14,
                     finalized_at = $15,
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
            .bind::<Nullable<SqlUuid>, _>(self.image_asset_id)
            .bind::<Nullable<Timestamptz>, _>(self.timeout_start_at)
            .bind::<Nullable<Timestamptz>, _>(self.timeout_at)
            .bind::<Timestamp, _>(self.interaction_date)
            .bind::<Text, _>(self.trace_type.to_db())
            .bind::<Text, _>(self.status.to_db())
            .bind::<Nullable<SqlUuid>, _>(self.journal_id)
            .bind::<Timestamp, _>(self.start_writing_at)
            .bind::<Nullable<Timestamp>, _>(self.finalized_at)
            .execute(conn)?;

            sync_trace_image_asset_to_related_posts(conn, self.id, self.image_asset_id)?;

            if let Some(journal_id) = self.journal_id {
                recalculate_journal_last_trace_at(conn, journal_id)?;
            }
            Ok(())
        })?;

        Trace::find_full_trace(self.id, pool)
    }

    pub fn set_status(mut self, status: TraceStatus, pool: &DbPool) -> Result<Trace, PpdcError> {
        self.status = status;
        match status {
            TraceStatus::Draft => {
                self.finalized_at = None;
            }
            TraceStatus::Finalized | TraceStatus::Archived => {
                self.timeout_at = None;
                if self.finalized_at.is_none() {
                    self.finalized_at = Some(Utc::now().naive_utc());
                }
            }
        }
        self.update(pool)
    }
}

impl NewTrace {
    pub fn create(mut self, pool: &DbPool) -> Result<Trace, PpdcError> {
        if self.is_encrypted && self.encryption_metadata.is_none() {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "encryption_metadata is required when is_encrypted is true".to_string(),
            ));
        }
        if self.timeout_at.is_some() && self.timeout_start_at.is_none() {
            self.timeout_start_at = Some(Utc::now());
        }
        let mut conn = pool
            .get()?;

        let inserted = conn.transaction::<IdRow, diesel::result::Error, _>(|conn| {
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
                    image_asset_id,
                    timeout_start_at,
                    timeout_at,
                    interaction_date,
                    trace_type,
                    status,
                    start_writing_at,
                    finalized_at
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
                    $10,
                    $11,
                    $12,
                    'DRAFT',
                    $13,
                    NULL
                 )
                 RETURNING id",
            )
            .bind::<SqlUuid, _>(self.user_id)
            .bind::<SqlUuid, _>(self.journal_id)
            .bind::<Text, _>(&self.title)
            .bind::<Text, _>(&self.subtitle)
            .bind::<Text, _>(&self.content)
            .bind::<Bool, _>(self.is_encrypted)
            .bind::<Nullable<Text>, _>(
                self.encryption_metadata
                    .as_ref()
                    .map(|value| value.to_string()),
            )
            .bind::<Nullable<SqlUuid>, _>(self.image_asset_id)
            .bind::<Nullable<Timestamptz>, _>(self.timeout_start_at)
            .bind::<Nullable<Timestamptz>, _>(self.timeout_at)
            .bind::<Timestamp, _>(self.interaction_date)
            .bind::<Text, _>(self.trace_type.to_db())
            .bind::<Timestamp, _>(self.start_writing_at)
            .get_result::<IdRow>(conn)?;

            recalculate_journal_last_trace_at(conn, self.journal_id)?;

            Ok(inserted)
        })?;

        Trace::find_full_trace(inserted.id, pool)
    }
}
