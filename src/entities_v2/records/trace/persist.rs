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

#[derive(QueryableByName)]
struct OptionalIdRow {
    #[diesel(sql_type = Nullable<SqlUuid>)]
    id: Option<uuid::Uuid>,
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

fn sync_draft_trace_version_from_trace(
    conn: &mut diesel::PgConnection,
    trace: &Trace,
) -> Result<(), diesel::result::Error> {
    if trace.status != TraceStatus::Draft {
        return Ok(());
    }

    diesel::sql_query(
        "INSERT INTO trace_versions (
            id,
            trace_id,
            version_number,
            status,
            title,
            subtitle,
            content,
            image_asset_id,
            interaction_date,
            created_at,
            updated_at,
            finalized_at
         ) VALUES (
            uuid_generate_v4(),
            $1,
            NULL,
            'DRAFT',
            $2,
            $3,
            $4,
            $5,
            $6,
            NOW(),
            NOW(),
            NULL
         )
         ON CONFLICT (trace_id) WHERE status = 'DRAFT'
         DO UPDATE SET
            title = EXCLUDED.title,
            subtitle = EXCLUDED.subtitle,
            content = EXCLUDED.content,
            image_asset_id = EXCLUDED.image_asset_id,
            interaction_date = EXCLUDED.interaction_date,
            updated_at = NOW()",
    )
    .bind::<SqlUuid, _>(trace.id)
    .bind::<Text, _>(&trace.title)
    .bind::<Text, _>(&trace.subtitle)
    .bind::<Text, _>(&trace.content)
    .bind::<Nullable<SqlUuid>, _>(trace.image_asset_id)
    .bind::<Timestamp, _>(trace.interaction_date)
    .execute(conn)?;

    Ok(())
}

fn finalize_draft_trace_version_from_trace(
    conn: &mut diesel::PgConnection,
    trace: &Trace,
) -> Result<(), diesel::result::Error> {
    if trace.status != TraceStatus::Finalized {
        return Ok(());
    }

    let current = diesel::sql_query(
        "SELECT current_version_id AS id
         FROM traces
         WHERE id = $1",
    )
    .bind::<SqlUuid, _>(trace.id)
    .get_result::<OptionalIdRow>(conn)?;
    if current.id.is_some() {
        return Ok(());
    }

    let finalized_at = trace.finalized_at.unwrap_or_else(|| Utc::now().naive_utc());
    let finalized = diesel::sql_query(
        "WITH next_version AS (
            SELECT COALESCE(MAX(version_number), 0) + 1 AS version_number
            FROM trace_versions
            WHERE trace_id = $1
              AND status = 'FINALIZED'
         ),
         updated_draft AS (
            UPDATE trace_versions
            SET status = 'FINALIZED',
                version_number = (SELECT version_number FROM next_version),
                title = $2,
                subtitle = $3,
                content = $4,
                image_asset_id = $5,
                interaction_date = $6,
                updated_at = NOW(),
                finalized_at = $7
            WHERE id = (
                SELECT id
                FROM trace_versions
                WHERE trace_id = $1
                  AND status = 'DRAFT'
                LIMIT 1
            )
            RETURNING id
         ),
         inserted_version AS (
            INSERT INTO trace_versions (
                id,
                trace_id,
                version_number,
                status,
                title,
                subtitle,
                content,
                image_asset_id,
                interaction_date,
                created_at,
                updated_at,
                finalized_at
            )
            SELECT
                uuid_generate_v4(),
                $1,
                (SELECT version_number FROM next_version),
                'FINALIZED',
                $2,
                $3,
                $4,
                $5,
                $6,
                NOW(),
                NOW(),
                $7
            WHERE NOT EXISTS (SELECT 1 FROM updated_draft)
            RETURNING id
         )
         SELECT id FROM updated_draft
         UNION ALL
         SELECT id FROM inserted_version
         LIMIT 1",
    )
    .bind::<SqlUuid, _>(trace.id)
    .bind::<Text, _>(&trace.title)
    .bind::<Text, _>(&trace.subtitle)
    .bind::<Text, _>(&trace.content)
    .bind::<Nullable<SqlUuid>, _>(trace.image_asset_id)
    .bind::<Timestamp, _>(trace.interaction_date)
    .bind::<Timestamp, _>(finalized_at)
    .get_result::<IdRow>(conn)?;

    diesel::sql_query(
        "UPDATE traces
         SET current_version_id = $2
         WHERE id = $1",
    )
    .bind::<SqlUuid, _>(trace.id)
    .bind::<SqlUuid, _>(finalized.id)
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
         WHERE source_trace_id = $1
           AND status = 'DRAFT'
           AND content_source = 'TRACE_VERSION'",
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
        let mut conn = pool.get()?;

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
            sync_draft_trace_version_from_trace(conn, &self)?;
            finalize_draft_trace_version_from_trace(conn, &self)?;

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
        let mut conn = pool.get()?;

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
            diesel::sql_query(
                "INSERT INTO trace_versions (
                    id,
                    trace_id,
                    version_number,
                    status,
                    title,
                    subtitle,
                    content,
                    image_asset_id,
                    interaction_date,
                    created_at,
                    updated_at,
                    finalized_at
                 ) VALUES (
                    uuid_generate_v4(),
                    $1,
                    NULL,
                    'DRAFT',
                    $2,
                    $3,
                    $4,
                    $5,
                    $6,
                    NOW(),
                    NOW(),
                    NULL
                 )",
            )
            .bind::<SqlUuid, _>(inserted.id)
            .bind::<Text, _>(&self.title)
            .bind::<Text, _>(&self.subtitle)
            .bind::<Text, _>(&self.content)
            .bind::<Nullable<SqlUuid>, _>(self.image_asset_id)
            .bind::<Timestamp, _>(self.interaction_date)
            .execute(conn)?;

            Ok(inserted)
        })?;

        Trace::find_full_trace(inserted.id, pool)
    }
}
