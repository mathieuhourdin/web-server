use chrono::Utc;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Bool, Nullable, Text, Timestamp, Timestamptz, Uuid as SqlUuid};

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::journal::{Journal, JournalStatus, JournalType};
use crate::entities_v2::post::{enforce_publication_invariant_for_source, PostSourceRef};
use crate::entities_v2::trace_search::TraceSearchDocument;
use crate::schema::trace_attachments;

use super::model::{NewTrace, Trace, TraceStatus};

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: uuid::Uuid,
}

#[derive(QueryableByName)]
struct AffectedRows {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct VersionIntegerRow {
    #[diesel(sql_type = diesel::sql_types::Int4)]
    version_integer: i32,
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
               AND (
                    status <> 'DRAFT'
                    OR trace_type <> 'USER_TRACE'
                    OR is_blank = FALSE
               )
         ),
         updated_at = NOW()
         WHERE id = $1",
    )
    .bind::<SqlUuid, _>(journal_id)
    .execute(conn)?;
    Ok(())
}

fn trace_has_attachments(
    conn: &mut diesel::PgConnection,
    trace_id: uuid::Uuid,
) -> Result<bool, diesel::result::Error> {
    let exists = diesel::select(diesel::dsl::exists(
        trace_attachments::table.filter(trace_attachments::trace_id.eq(trace_id)),
    ))
    .get_result::<bool>(conn)?;
    Ok(exists)
}

fn compute_is_blank_with_conn(
    conn: &mut diesel::PgConnection,
    trace: &Trace,
) -> Result<bool, diesel::result::Error> {
    let has_attachments = trace_has_attachments(conn, trace.id)?;
    Ok(trace.title.trim().is_empty()
        && trace.content.trim().is_empty()
        && trace.content_image_asset_id.is_none()
        && !has_attachments)
}

impl Trace {
    pub fn update(self, pool: &DbPool) -> Result<Trace, PpdcError> {
        self.update_internal(None, pool)
    }

    pub fn update_with_expected_version(
        self,
        expected_version_integer: i32,
        pool: &DbPool,
    ) -> Result<Trace, PpdcError> {
        self.update_internal(Some(expected_version_integer), pool)
    }

    fn update_internal(
        mut self,
        expected_version_integer: Option<i32>,
        pool: &DbPool,
    ) -> Result<Trace, PpdcError> {
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

        let updated = conn.transaction::<bool, diesel::result::Error, _>(|conn| {
            let previous_is_blank = self.is_blank;
            self.is_blank = compute_is_blank_with_conn(conn, &self)?;
            if previous_is_blank && !self.is_blank {
                self.start_writing_at = Utc::now().naive_utc();
            }
            let rows_affected = if let Some(expected_version_integer) = expected_version_integer {
                diesel::sql_query(
                    "WITH updated AS (
                        UPDATE traces
                        SET title = $2,
                            subtitle = $3,
                            content = $4,
                            derived_from_trace_id = $5,
                            is_encrypted = $6,
                            encryption_metadata = CAST($7 AS jsonb),
                            content_image_asset_id = $8,
                            sharing_sensitivity = $9,
                            timeout_start_at = $10,
                            timeout_at = $11,
                            interaction_date = $12,
                            trace_type = $13,
                            status = $14,
                            journal_id = $15,
                            is_blank = $16,
                            start_writing_at = $17,
                            finalized_at = $18,
                            version_integer = version_integer + 1,
                            updated_at = NOW()
                        WHERE id = $1
                          AND version_integer = $19
                        RETURNING 1
                    )
                    SELECT COUNT(*)::bigint AS count FROM updated",
                )
                .bind::<SqlUuid, _>(self.id)
                .bind::<Text, _>(&self.title)
                .bind::<Text, _>(&self.subtitle)
                .bind::<Text, _>(&self.content)
                .bind::<Nullable<SqlUuid>, _>(self.derived_from_trace_id)
                .bind::<Bool, _>(self.is_encrypted)
                .bind::<Nullable<Text>, _>(
                    self.encryption_metadata
                        .as_ref()
                        .map(|value| value.to_string()),
                )
                .bind::<Nullable<SqlUuid>, _>(self.content_image_asset_id)
                .bind::<Text, _>(self.sharing_sensitivity.to_db())
                .bind::<Nullable<Timestamptz>, _>(self.timeout_start_at)
                .bind::<Nullable<Timestamptz>, _>(self.timeout_at)
                .bind::<Timestamp, _>(self.interaction_date)
                .bind::<Text, _>(self.trace_type.to_db())
                .bind::<Text, _>(self.status.to_db())
                .bind::<Nullable<SqlUuid>, _>(self.journal_id)
                .bind::<Bool, _>(self.is_blank)
                .bind::<Timestamp, _>(self.start_writing_at)
                .bind::<Nullable<Timestamp>, _>(self.finalized_at)
                .bind::<diesel::sql_types::Int4, _>(expected_version_integer)
                .get_result::<AffectedRows>(conn)?
                .count
            } else {
                diesel::sql_query(
                    "WITH updated AS (
                        UPDATE traces
                        SET title = $2,
                            subtitle = $3,
                            content = $4,
                            derived_from_trace_id = $5,
                            is_encrypted = $6,
                            encryption_metadata = CAST($7 AS jsonb),
                            content_image_asset_id = $8,
                            sharing_sensitivity = $9,
                            timeout_start_at = $10,
                            timeout_at = $11,
                            interaction_date = $12,
                            trace_type = $13,
                            status = $14,
                            journal_id = $15,
                            is_blank = $16,
                            start_writing_at = $17,
                            finalized_at = $18,
                            version_integer = version_integer + 1,
                            updated_at = NOW()
                        WHERE id = $1
                        RETURNING 1
                    )
                    SELECT COUNT(*)::bigint AS count FROM updated",
                )
                .bind::<SqlUuid, _>(self.id)
                .bind::<Text, _>(&self.title)
                .bind::<Text, _>(&self.subtitle)
                .bind::<Text, _>(&self.content)
                .bind::<Nullable<SqlUuid>, _>(self.derived_from_trace_id)
                .bind::<Bool, _>(self.is_encrypted)
                .bind::<Nullable<Text>, _>(
                    self.encryption_metadata
                        .as_ref()
                        .map(|value| value.to_string()),
                )
                .bind::<Nullable<SqlUuid>, _>(self.content_image_asset_id)
                .bind::<Text, _>(self.sharing_sensitivity.to_db())
                .bind::<Nullable<Timestamptz>, _>(self.timeout_start_at)
                .bind::<Nullable<Timestamptz>, _>(self.timeout_at)
                .bind::<Timestamp, _>(self.interaction_date)
                .bind::<Text, _>(self.trace_type.to_db())
                .bind::<Text, _>(self.status.to_db())
                .bind::<Nullable<SqlUuid>, _>(self.journal_id)
                .bind::<Bool, _>(self.is_blank)
                .bind::<Timestamp, _>(self.start_writing_at)
                .bind::<Nullable<Timestamp>, _>(self.finalized_at)
                .get_result::<AffectedRows>(conn)?
                .count
            };

            if rows_affected == 0 {
                return Ok(false);
            }

            enforce_publication_invariant_for_source(
                PostSourceRef::Trace(self.id),
                self.status.permits_published_post(),
                conn,
            )?;

            if let Some(journal_id) = self.journal_id {
                recalculate_journal_last_trace_at(conn, journal_id)?;
            }
            Ok(true)
        })?;

        if !updated {
            let current_version_integer = {
                let mut conn = pool.get()?;
                diesel::sql_query(
                    "SELECT version_integer
                     FROM traces
                     WHERE id = $1",
                )
                .bind::<SqlUuid, _>(self.id)
                .get_result::<VersionIntegerRow>(&mut conn)?
                .version_integer
            };

            return Err(PpdcError::new(
                409,
                ErrorType::ApiError,
                "Trace was updated by another request".to_string(),
            )
            .with_details(serde_json::json!({
                "code": "trace_version_conflict",
                "expected_version_integer": expected_version_integer,
                "current_version_integer": current_version_integer,
            })));
        }

        if self.status != TraceStatus::Draft {
            TraceSearchDocument::refresh_for_trace(self.id, pool)?;
        }

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
        self.is_blank = self.title.trim().is_empty()
            && self.content.trim().is_empty()
            && self.content_image_asset_id.is_none();
        let mut conn = pool.get()?;

        let inserted = conn.transaction::<IdRow, diesel::result::Error, _>(|conn| {
            let inserted = diesel::sql_query(
                "INSERT INTO traces (
                    id,
                    user_id,
                    journal_id,
                    derived_from_trace_id,
                    title,
                    subtitle,
                    content,
                    is_encrypted,
                    encryption_metadata,
                    content_image_asset_id,
                    sharing_sensitivity,
                    timeout_start_at,
                    timeout_at,
                    interaction_date,
                    trace_type,
                    version_integer,
                    status,
                    is_blank,
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
                    $7,
                    CAST($8 AS jsonb),
                    $9,
                    $10,
                    $11,
                    $12,
                    $13,
                    $14,
                    0,
                    'DRAFT',
                    $15,
                    $16,
                    NULL
                 )
                 RETURNING id",
            )
            .bind::<SqlUuid, _>(self.user_id)
            .bind::<SqlUuid, _>(self.journal_id)
            .bind::<Nullable<SqlUuid>, _>(self.derived_from_trace_id)
            .bind::<Text, _>(&self.title)
            .bind::<Text, _>(&self.subtitle)
            .bind::<Text, _>(&self.content)
            .bind::<Bool, _>(self.is_encrypted)
            .bind::<Nullable<Text>, _>(
                self.encryption_metadata
                    .as_ref()
                    .map(|value| value.to_string()),
            )
            .bind::<Nullable<SqlUuid>, _>(self.content_image_asset_id)
            .bind::<Text, _>(self.sharing_sensitivity.to_db())
            .bind::<Nullable<Timestamptz>, _>(self.timeout_start_at)
            .bind::<Nullable<Timestamptz>, _>(self.timeout_at)
            .bind::<Timestamp, _>(self.interaction_date)
            .bind::<Text, _>(self.trace_type.to_db())
            .bind::<Bool, _>(self.is_blank)
            .bind::<Timestamp, _>(self.start_writing_at)
            .get_result::<IdRow>(conn)?;

            recalculate_journal_last_trace_at(conn, self.journal_id)?;

            Ok(inserted)
        })?;

        Trace::find_full_trace(inserted.id, pool)
    }

    pub fn create_finalized(mut self, pool: &DbPool) -> Result<Trace, PpdcError> {
        if self.is_encrypted && self.encryption_metadata.is_none() {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "encryption_metadata is required when is_encrypted is true".to_string(),
            ));
        }
        self.timeout_start_at = None;
        self.timeout_at = None;
        self.is_blank = false;
        let finalized_at = Utc::now().naive_utc();
        let mut conn = pool.get()?;

        let inserted = conn.transaction::<IdRow, diesel::result::Error, _>(|conn| {
            let inserted = diesel::sql_query(
                "INSERT INTO traces (
                    id,
                    user_id,
                    journal_id,
                    derived_from_trace_id,
                    title,
                    subtitle,
                    content,
                    is_encrypted,
                    encryption_metadata,
                    content_image_asset_id,
                    sharing_sensitivity,
                    timeout_start_at,
                    timeout_at,
                    interaction_date,
                    trace_type,
                    version_integer,
                    status,
                    is_blank,
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
                    $7,
                    CAST($8 AS jsonb),
                    $9,
                    $10,
                    $11,
                    $12,
                    $13,
                    $14,
                    0,
                    'FINALIZED',
                    FALSE,
                    $15,
                    $16
                 )
                 RETURNING id",
            )
            .bind::<SqlUuid, _>(self.user_id)
            .bind::<SqlUuid, _>(self.journal_id)
            .bind::<Nullable<SqlUuid>, _>(self.derived_from_trace_id)
            .bind::<Text, _>(&self.title)
            .bind::<Text, _>(&self.subtitle)
            .bind::<Text, _>(&self.content)
            .bind::<Bool, _>(self.is_encrypted)
            .bind::<Nullable<Text>, _>(
                self.encryption_metadata
                    .as_ref()
                    .map(|value| value.to_string()),
            )
            .bind::<Nullable<SqlUuid>, _>(self.content_image_asset_id)
            .bind::<Text, _>(self.sharing_sensitivity.to_db())
            .bind::<Nullable<Timestamptz>, _>(self.timeout_start_at)
            .bind::<Nullable<Timestamptz>, _>(self.timeout_at)
            .bind::<Timestamp, _>(self.interaction_date)
            .bind::<Text, _>(self.trace_type.to_db())
            .bind::<Timestamp, _>(self.start_writing_at)
            .bind::<Timestamp, _>(finalized_at)
            .get_result::<IdRow>(conn)?;

            recalculate_journal_last_trace_at(conn, self.journal_id)?;

            Ok(inserted)
        })?;

        let trace = Trace::find_full_trace(inserted.id, pool)?;
        TraceSearchDocument::refresh_for_trace(trace.id, pool)?;
        Ok(trace)
    }
}

impl Trace {
    pub fn create_blank_draft_for_journal(
        journal: &Journal,
        pool: &DbPool,
    ) -> Result<Trace, PpdcError> {
        if journal.status != JournalStatus::Active
            || journal.journal_type != JournalType::UserJournal
        {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Blank drafts are only created for active user journals".to_string(),
            ));
        }
        let mut trace = NewTrace::new(
            String::new(),
            String::new(),
            String::new(),
            Utc::now().naive_utc(),
            journal.user_id,
            journal.id,
        );
        trace.is_blank = true;
        trace.create(pool)
    }
}
