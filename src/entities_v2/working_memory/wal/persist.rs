use std::collections::HashSet;

use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Date, Int4, Jsonb, Text, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::{wal_days, wal_entries, wal_projections};

use super::model::{
    NewWalDay, NewWalEntry, WalCarryoverApplicationStatus, WalCarryoverContent,
    WalCompilationViews, WalDay, WalEntry, WalProjection, WalProjectionStatus, WalProjectionType,
    WalProjectionView, WalStructuredProjection,
};

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

#[derive(QueryableByName)]
struct DueWalDayRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

impl WalDay {
    pub(crate) fn get_or_create_with_conn(
        user_id: Uuid,
        local_date: NaiveDate,
        conn: &mut PgConnection,
    ) -> Result<Self, PpdcError> {
        let new_wal = NewWalDay {
            id: Uuid::new_v4(),
            user_id,
            local_date,
            input: String::new(),
            context: String::new(),
        };
        diesel::insert_into(wal_days::table)
            .values(&new_wal)
            .on_conflict((wal_days::user_id, wal_days::local_date))
            .do_nothing()
            .execute(conn)?;

        Ok(wal_days::table
            .filter(wal_days::user_id.eq(user_id))
            .filter(wal_days::local_date.eq(local_date))
            .select(WalDay::as_select())
            .first(conn)?)
    }

    pub fn get_or_create(
        user_id: Uuid,
        local_date: NaiveDate,
        pool: &DbPool,
    ) -> Result<Self, PpdcError> {
        let mut conn = pool.get()?;
        conn.transaction(|conn| Self::get_or_create_with_conn(user_id, local_date, conn))
    }

    pub fn find_for_user_and_date(
        user_id: Uuid,
        local_date: NaiveDate,
        pool: &DbPool,
    ) -> Result<Option<Self>, PpdcError> {
        let mut conn = pool.get()?;
        Ok(wal_days::table
            .filter(wal_days::user_id.eq(user_id))
            .filter(wal_days::local_date.eq(local_date))
            .select(WalDay::as_select())
            .first::<Self>(&mut conn)
            .optional()?)
    }

    fn append_entry_with_conn(
        current: Self,
        entry: String,
        schedule_compilation: bool,
        conn: &mut PgConnection,
    ) -> Result<(Self, WalEntry), PpdcError> {
        let next_position = wal_entries::table
            .filter(wal_entries::wal_day_id.eq(current.id))
            .select(diesel::dsl::max(wal_entries::position))
            .first::<Option<i32>>(conn)?
            .map(|position| position + 1)
            .unwrap_or(0);
        let new_entry = NewWalEntry {
            id: Uuid::new_v4(),
            wal_day_id: current.id,
            position: next_position,
            content: entry.clone(),
        };
        let created_entry = diesel::insert_into(wal_entries::table)
            .values(&new_entry)
            .returning(WalEntry::as_returning())
            .get_result::<WalEntry>(conn)?;
        let input = if current.input.is_empty() || current.input.ends_with('\n') {
            format!("{}{}", current.input, entry)
        } else {
            format!("{}\n{}", current.input, entry)
        };

        let updated =
            diesel::update(wal_days::table.filter(wal_days::id.eq(current.id)))
                .set((
                    wal_days::input.eq(input),
                    wal_days::input_revision.eq(current.input_revision + 1),
                    wal_days::compiled_operational.eq::<Option<String>>(None),
                    wal_days::compiled_thematic.eq::<Option<String>>(None),
                    wal_days::compiled_at.eq::<Option<NaiveDateTime>>(None),
                    wal_days::compilation_due_at
                        .eq(schedule_compilation
                            .then(|| Utc::now().naive_utc() + Duration::minutes(1))),
                    wal_days::compilation_last_error.eq::<Option<String>>(None),
                    wal_days::updated_at.eq(diesel::dsl::now),
                ))
                .returning(WalDay::as_returning())
                .get_result::<WalDay>(conn)?;

        diesel::update(
            wal_projections::table
                .filter(wal_projections::wal_day_id.eq(current.id))
                .filter(wal_projections::projection_type.eq_any([
                    WalProjectionType::Operational.to_db(),
                    WalProjectionType::Thematic.to_db(),
                ])),
        )
        .set(wal_projections::status.eq(WalProjectionStatus::Stale.to_db()))
        .execute(conn)?;

        Ok((updated, created_entry))
    }

    pub fn append(
        user_id: Uuid,
        local_date: NaiveDate,
        entry: String,
        schedule_compilation: bool,
        pool: &DbPool,
    ) -> Result<(Self, WalEntry), PpdcError> {
        let mut conn = pool.get()?;
        conn.transaction::<_, PpdcError, _>(|conn| {
            Self::get_or_create_with_conn(user_id, local_date, conn)?;
            let current = wal_days::table
                .filter(wal_days::user_id.eq(user_id))
                .filter(wal_days::local_date.eq(local_date))
                .for_update()
                .select(WalDay::as_select())
                .first::<WalDay>(conn)?;
            Self::append_entry_with_conn(current, entry, schedule_compilation, conn)
        })
    }

    pub fn entries(&self, pool: &DbPool) -> Result<Vec<WalEntry>, PpdcError> {
        WalEntry::find_for_day(self.id, pool)
    }

    pub fn find_for_user_paginated(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Self>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let total = wal_days::table
            .filter(wal_days::user_id.eq(user_id))
            .count()
            .get_result::<i64>(&mut conn)?;
        let items = wal_days::table
            .filter(wal_days::user_id.eq(user_id))
            .order((wal_days::local_date.desc(), wal_days::created_at.desc()))
            .offset(offset)
            .limit(limit)
            .select(WalDay::as_select())
            .load::<WalDay>(&mut conn)?;
        Ok((items, total))
    }

    pub fn find_due_for_carryover(limit: i64, pool: &DbPool) -> Result<Vec<Self>, PpdcError> {
        let mut conn = pool.get()?;
        let ids = sql_query(
            r#"
            SELECT wd.id
            FROM wal_days wd
            INNER JOIN users u ON u.id = wd.user_id
            WHERE BTRIM(wd.input) <> ''
              AND u.ai_features_enabled = TRUE
              AND u.ai_features_enabled_by_admin = TRUE
              AND wd.local_date = (
                    timezone(
                        COALESCE(NULLIF(CASE
                            WHEN u.timezone = 'Asia/Saigon' THEN 'Asia/Ho_Chi_Minh'
                            WHEN u.timezone IN (SELECT name FROM pg_timezone_names) THEN u.timezone
                            ELSE 'UTC'
                        END, ''), 'UTC'),
                        NOW()
                    )::date - 1
                  )
              AND NOT EXISTS (
                    SELECT 1
                    FROM wal_projections wp
                    WHERE wp.wal_day_id = wd.id
                      AND wp.projection_type = 'CARRYOVER'
                      AND wp.target_date = wd.local_date + 1
                  )
            ORDER BY wd.local_date, wd.created_at
            LIMIT $1
            "#,
        )
        .bind::<BigInt, _>(limit)
        .load::<DueWalDayRow>(&mut conn)?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ordered_ids = ids.into_iter().map(|row| row.id).collect::<Vec<_>>();
        let rows = wal_days::table
            .filter(wal_days::id.eq_any(&ordered_ids))
            .select(WalDay::as_select())
            .load::<WalDay>(&mut conn)?;
        let by_id = rows
            .into_iter()
            .map(|row| (row.id, row))
            .collect::<std::collections::HashMap<_, _>>();
        Ok(ordered_ids
            .into_iter()
            .filter_map(|id| by_id.get(&id).cloned())
            .collect())
    }

    pub fn claim_due_compilations(
        limit: i64,
        lease_seconds: i64,
        pool: &DbPool,
    ) -> Result<Vec<Self>, PpdcError> {
        if limit <= 0 {
            return Ok(Vec::new());
        }
        let mut conn = pool.get()?;
        conn.transaction::<Vec<Self>, PpdcError, _>(|conn| {
            let claimed_ids = sql_query(
                r#"
                WITH candidates AS (
                    SELECT wd.id
                    FROM wal_days wd
                    INNER JOIN users u ON u.id = wd.user_id
                    WHERE BTRIM(wd.input) <> ''
                      AND u.ai_features_enabled = TRUE
                      AND u.ai_features_enabled_by_admin = TRUE
                      AND (
                            (
                                wd.compilation_due_at <= NOW()
                                AND wd.compilation_processing_revision IS NULL
                            )
                            OR
                            (
                                wd.compilation_processing_revision IS NOT NULL
                                AND wd.compilation_started_at <= NOW() - make_interval(secs => $2::double precision)
                            )
                          )
                    ORDER BY COALESCE(wd.compilation_due_at, wd.compilation_started_at), wd.id
                    FOR UPDATE OF wd SKIP LOCKED
                    LIMIT $1
                ), claimed AS (
                    UPDATE wal_days wd
                    SET compilation_due_at = NULL,
                        compilation_started_at = NOW(),
                        compilation_processing_revision = wd.input_revision,
                        compilation_last_error = NULL,
                        updated_at = NOW()
                    FROM candidates c
                    WHERE wd.id = c.id
                    RETURNING wd.id
                )
                SELECT id FROM claimed
                "#,
            )
            .bind::<BigInt, _>(limit)
            .bind::<BigInt, _>(lease_seconds)
            .load::<IdRow>(conn)?
            .into_iter()
            .map(|row| row.id)
            .collect::<Vec<_>>();

            if claimed_ids.is_empty() {
                return Ok(Vec::new());
            }
            diesel::update(
                wal_projections::table
                    .filter(wal_projections::wal_day_id.eq_any(&claimed_ids))
                    .filter(wal_projections::projection_type.eq_any([
                        WalProjectionType::Operational.to_db(),
                        WalProjectionType::Thematic.to_db(),
                    ])),
            )
            .set((
                wal_projections::status.eq(WalProjectionStatus::Processing.to_db()),
                wal_projections::error_message.eq::<Option<String>>(None),
                wal_projections::updated_at.eq(diesel::dsl::now),
            ))
            .execute(conn)?;

            Ok(wal_days::table
                .filter(wal_days::id.eq_any(claimed_ids))
                .select(Self::as_select())
                .load::<Self>(conn)?)
        })
    }

    pub fn claim_for_immediate_compilation(
        &self,
        lease_seconds: i64,
        pool: &DbPool,
    ) -> Result<Self, PpdcError> {
        let mut conn = pool.get()?;
        conn.transaction::<Self, PpdcError, _>(|conn| {
            let current = wal_days::table
                .filter(wal_days::id.eq(self.id))
                .filter(wal_days::user_id.eq(self.user_id))
                .for_update()
                .select(Self::as_select())
                .first::<Self>(conn)?;
            let claim_is_active = current
                .compilation_started_at
                .map(|started_at| {
                    started_at > Utc::now().naive_utc() - Duration::seconds(lease_seconds)
                })
                .unwrap_or(false)
                && current.compilation_processing_revision.is_some();
            if claim_is_active {
                return Err(PpdcError::new(
                    409,
                    ErrorType::ApiError,
                    "WAL compilation is already processing".to_string(),
                )
                .with_details(serde_json::json!({
                    "code": "wal_compilation_already_processing",
                    "processing_revision": current.compilation_processing_revision,
                })));
            }

            let claimed = diesel::update(wal_days::table.filter(wal_days::id.eq(current.id)))
                .set((
                    wal_days::compilation_due_at.eq::<Option<NaiveDateTime>>(None),
                    wal_days::compilation_started_at.eq(Some(Utc::now().naive_utc())),
                    wal_days::compilation_processing_revision.eq(Some(current.input_revision)),
                    wal_days::compilation_last_error.eq::<Option<String>>(None),
                    wal_days::updated_at.eq(diesel::dsl::now),
                ))
                .returning(Self::as_returning())
                .get_result::<Self>(conn)?;
            diesel::update(
                wal_projections::table
                    .filter(wal_projections::wal_day_id.eq(current.id))
                    .filter(wal_projections::projection_type.eq_any([
                        WalProjectionType::Operational.to_db(),
                        WalProjectionType::Thematic.to_db(),
                    ])),
            )
            .set((
                wal_projections::status.eq(WalProjectionStatus::Processing.to_db()),
                wal_projections::error_message.eq::<Option<String>>(None),
                wal_projections::updated_at.eq(diesel::dsl::now),
            ))
            .execute(conn)?;
            Ok(claimed)
        })
    }

    pub fn release_compilation_claim(
        &self,
        error_message: Option<&str>,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        let Some(claimed_revision) = self.compilation_processing_revision else {
            return Ok(());
        };
        let mut conn = pool.get()?;
        conn.transaction::<(), PpdcError, _>(|conn| {
            let affected = diesel::update(
                wal_days::table
                    .filter(wal_days::id.eq(self.id))
                    .filter(wal_days::compilation_processing_revision.eq(Some(claimed_revision))),
            )
            .set((
                wal_days::compilation_started_at.eq::<Option<NaiveDateTime>>(None),
                wal_days::compilation_processing_revision.eq::<Option<i64>>(None),
                wal_days::compilation_last_error.eq(error_message.map(str::to_string)),
                wal_days::updated_at.eq(diesel::dsl::now),
            ))
            .execute(conn)?;

            if affected > 0 && error_message.is_some() {
                diesel::update(
                    wal_projections::table
                        .filter(wal_projections::wal_day_id.eq(self.id))
                        .filter(wal_projections::projection_type.eq_any([
                            WalProjectionType::Operational.to_db(),
                            WalProjectionType::Thematic.to_db(),
                        ])),
                )
                .set((
                    wal_projections::status.eq(WalProjectionStatus::Failed.to_db()),
                    wal_projections::error_message.eq(error_message.map(str::to_string)),
                    wal_projections::updated_at.eq(diesel::dsl::now),
                ))
                .execute(conn)?;
            }
            Ok(())
        })
    }

    pub fn save_compilations_if_unchanged(
        &self,
        operational_content: String,
        thematic_content: String,
        operational_projection: &WalStructuredProjection,
        thematic_projection: &WalStructuredProjection,
        prompt_version: &str,
        pool: &DbPool,
    ) -> Result<WalCompilationViews, PpdcError> {
        let compiled_at = Utc::now().naive_utc();
        let mut conn = pool.get()?;
        conn.transaction::<_, PpdcError, _>(|conn| {
            let current = wal_days::table
                .filter(wal_days::id.eq(self.id))
                .filter(wal_days::user_id.eq(self.user_id))
                .for_update()
                .select(WalDay::as_select())
                .first::<WalDay>(conn)?;
            if current.input_revision != self.input_revision {
                return Err(PpdcError::new(
                    409,
                    ErrorType::ApiError,
                    "WAL content changed while compiling; retry the compilation".to_string(),
                ));
            }

            let updated = diesel::update(wal_days::table.filter(wal_days::id.eq(self.id)))
                .set((
                    wal_days::compiled_operational.eq(Some(operational_content)),
                    wal_days::compiled_thematic.eq(Some(thematic_content)),
                    wal_days::compiled_at.eq(Some(compiled_at)),
                    wal_days::compilation_due_at.eq::<Option<NaiveDateTime>>(None),
                    wal_days::compilation_started_at.eq::<Option<NaiveDateTime>>(None),
                    wal_days::compilation_processing_revision.eq::<Option<i64>>(None),
                    wal_days::compilation_last_error.eq::<Option<String>>(None),
                    wal_days::updated_at.eq(diesel::dsl::now),
                ))
                .returning(WalDay::as_returning())
                .get_result::<WalDay>(conn)?;

            WalProjection::upsert_compilation_with_conn(
                self,
                WalProjectionType::Operational,
                operational_projection,
                prompt_version,
                compiled_at,
                conn,
            )?;
            WalProjection::upsert_compilation_with_conn(
                self,
                WalProjectionType::Thematic,
                thematic_projection,
                prompt_version,
                compiled_at,
                conn,
            )?;

            WalProjection::compilation_views_with_conn(&updated, conn)
        })
    }
}

impl WalEntry {
    pub fn find_for_day(wal_day_id: Uuid, pool: &DbPool) -> Result<Vec<Self>, PpdcError> {
        let mut conn = pool.get()?;
        Ok(wal_entries::table
            .filter(wal_entries::wal_day_id.eq(wal_day_id))
            .order((wal_entries::position.asc(), wal_entries::created_at.asc()))
            .select(Self::as_select())
            .load::<Self>(&mut conn)?)
    }
}

impl WalProjection {
    fn upsert_compilation_with_conn(
        wal: &WalDay,
        projection_type: WalProjectionType,
        content: &WalStructuredProjection,
        prompt_version: &str,
        generated_at: NaiveDateTime,
        conn: &mut PgConnection,
    ) -> Result<(), PpdcError> {
        sql_query(
            r#"
            INSERT INTO wal_projections (
                id, wal_day_id, projection_type, target_date, status, source_revision,
                schema_version, prompt_version, content, error_message, generated_at,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, NULL, 'READY', $4, $5, $6, $7, NULL, $8, NOW(), NOW())
            ON CONFLICT (wal_day_id, projection_type) WHERE target_date IS NULL
            DO UPDATE SET
                status = 'READY',
                source_revision = EXCLUDED.source_revision,
                schema_version = EXCLUDED.schema_version,
                prompt_version = EXCLUDED.prompt_version,
                content = EXCLUDED.content,
                error_message = NULL,
                generated_at = EXCLUDED.generated_at,
                updated_at = NOW()
            "#,
        )
        .bind::<SqlUuid, _>(Uuid::new_v4())
        .bind::<SqlUuid, _>(wal.id)
        .bind::<Text, _>(projection_type.to_db())
        .bind::<BigInt, _>(wal.input_revision)
        .bind::<Int4, _>(content.schema_version)
        .bind::<Text, _>(prompt_version)
        .bind::<Jsonb, _>(serde_json::to_value(content)?)
        .bind::<diesel::sql_types::Timestamp, _>(generated_at)
        .execute(conn)?;
        Ok(())
    }

    fn compilation_views_with_conn(
        wal: &WalDay,
        conn: &mut PgConnection,
    ) -> Result<WalCompilationViews, PpdcError> {
        let projections = wal_projections::table
            .filter(wal_projections::wal_day_id.eq(wal.id))
            .filter(wal_projections::target_date.is_null())
            .select(Self::as_select())
            .load::<Self>(conn)?;
        let mut views = wal.legacy_compilation_views();
        for projection in projections {
            let Some(view) = projection.structured_view(wal.input_revision)? else {
                continue;
            };
            match WalProjectionType::from_db(&projection.projection_type)? {
                WalProjectionType::Operational => views.operational_projection = Some(view),
                WalProjectionType::Thematic => views.thematic_projection = Some(view),
                WalProjectionType::Carryover => {}
            }
        }
        Ok(views)
    }

    pub fn compilation_views(
        wal: &WalDay,
        pool: &DbPool,
    ) -> Result<WalCompilationViews, PpdcError> {
        let mut conn = pool.get()?;
        Self::compilation_views_with_conn(wal, &mut conn)
    }

    fn structured_view(
        &self,
        current_revision: i64,
    ) -> Result<Option<WalProjectionView>, PpdcError> {
        let Some(content) = self.content.clone() else {
            return Ok(None);
        };
        let content =
            serde_json::from_value::<WalStructuredProjection>(content).map_err(|error| {
                PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!("Invalid persisted WAL projection: {error}"),
                )
            })?;
        Ok(Some(WalProjectionView {
            id: self.id,
            status: WalProjectionStatus::from_db(&self.status)?,
            source_revision: self.source_revision,
            schema_version: self.schema_version,
            prompt_version: self.prompt_version.clone(),
            content,
            generated_at: self.generated_at,
            is_stale: self.source_revision != current_revision,
        }))
    }

    pub fn create_carryover_if_absent(
        wal: &WalDay,
        target_date: NaiveDate,
        prompt_version: &str,
        pool: &DbPool,
    ) -> Result<Option<Self>, PpdcError> {
        let mut conn = pool.get()?;
        let inserted = sql_query(
            r#"
            INSERT INTO wal_projections (
                id, wal_day_id, projection_type, target_date, status, source_revision,
                schema_version, prompt_version, created_at, updated_at
            )
            VALUES ($1, $2, 'CARRYOVER', $3, 'PROCESSING', $4, 1, $5, NOW(), NOW())
            ON CONFLICT (wal_day_id, projection_type, target_date) WHERE target_date IS NOT NULL
            DO NOTHING
            RETURNING id
            "#,
        )
        .bind::<SqlUuid, _>(Uuid::new_v4())
        .bind::<SqlUuid, _>(wal.id)
        .bind::<Date, _>(target_date)
        .bind::<BigInt, _>(wal.input_revision)
        .bind::<Text, _>(prompt_version)
        .get_result::<IdRow>(&mut conn)
        .optional()?;
        inserted
            .map(|row| {
                wal_projections::table
                    .filter(wal_projections::id.eq(row.id))
                    .select(Self::as_select())
                    .first::<Self>(&mut conn)
                    .map_err(PpdcError::from)
            })
            .transpose()
    }

    pub fn save_carryover_ready(
        &self,
        content: &WalCarryoverContent,
        pool: &DbPool,
    ) -> Result<Self, PpdcError> {
        let mut conn = pool.get()?;
        Ok(
            diesel::update(wal_projections::table.filter(wal_projections::id.eq(self.id)))
                .set((
                    wal_projections::status.eq(WalProjectionStatus::Ready.to_db()),
                    wal_projections::content.eq(Some(serde_json::to_value(content)?)),
                    wal_projections::error_message.eq::<Option<String>>(None),
                    wal_projections::generated_at.eq(Some(Utc::now().naive_utc())),
                ))
                .returning(Self::as_returning())
                .get_result::<Self>(&mut conn)?,
        )
    }

    pub fn mark_failed(&self, message: &str, pool: &DbPool) -> Result<(), PpdcError> {
        let mut conn = pool.get()?;
        diesel::update(wal_projections::table.filter(wal_projections::id.eq(self.id)))
            .set((
                wal_projections::status.eq(WalProjectionStatus::Failed.to_db()),
                wal_projections::error_message
                    .eq(Some(message.chars().take(1000).collect::<String>())),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn find_carryover(
        user_id: Uuid,
        source_date: NaiveDate,
        target_date: NaiveDate,
        pool: &DbPool,
    ) -> Result<Option<Self>, PpdcError> {
        let mut conn = pool.get()?;
        Ok(wal_projections::table
            .inner_join(wal_days::table)
            .filter(wal_days::user_id.eq(user_id))
            .filter(wal_days::local_date.eq(source_date))
            .filter(wal_projections::projection_type.eq(WalProjectionType::Carryover.to_db()))
            .filter(wal_projections::target_date.eq(Some(target_date)))
            .select(Self::as_select())
            .first::<Self>(&mut conn)
            .optional()?)
    }

    pub fn carryover_content(&self) -> Result<Option<WalCarryoverContent>, PpdcError> {
        self.content
            .clone()
            .map(|content| {
                serde_json::from_value(content).map_err(|error| {
                    PpdcError::new(
                        500,
                        ErrorType::InternalError,
                        format!("Invalid persisted WAL carryover: {error}"),
                    )
                })
            })
            .transpose()
    }

    pub fn apply_carryover_items(
        projection_id: Uuid,
        user_id: Uuid,
        target_date: NaiveDate,
        item_ids: &[Uuid],
        schedule_compilation: bool,
        pool: &DbPool,
    ) -> Result<WalDay, PpdcError> {
        let requested_ids = item_ids.iter().copied().collect::<HashSet<_>>();
        let mut conn = pool.get()?;
        conn.transaction::<_, PpdcError, _>(|conn| {
            let projection = wal_projections::table
                .filter(wal_projections::id.eq(projection_id))
                .for_update()
                .select(Self::as_select())
                .first::<Self>(conn)?;
            let _source_wal = wal_days::table
                .filter(wal_days::id.eq(projection.wal_day_id))
                .filter(wal_days::user_id.eq(user_id))
                .select(WalDay::as_select())
                .first::<WalDay>(conn)?;
            if projection.projection_type != WalProjectionType::Carryover.to_db()
                || projection.target_date != Some(target_date)
                || projection.status != WalProjectionStatus::Ready.to_db()
            {
                return Err(PpdcError::new(
                    409,
                    ErrorType::ApiError,
                    "This WAL carryover is not ready for the current day".to_string(),
                ));
            }
            let mut content = projection.carryover_content()?.ok_or_else(|| {
                PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    "Ready WAL carryover has no content".to_string(),
                )
            })?;
            let available_ids = content
                .items
                .iter()
                .map(|item| item.id)
                .collect::<HashSet<_>>();
            if !requested_ids.is_subset(&available_ids) {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "One or more carryover item IDs are invalid".to_string(),
                ));
            }

            WalDay::get_or_create_with_conn(user_id, target_date, conn)?;
            let mut target_wal = wal_days::table
                .filter(wal_days::user_id.eq(user_id))
                .filter(wal_days::local_date.eq(target_date))
                .for_update()
                .select(WalDay::as_select())
                .first::<WalDay>(conn)?;
            for item in content
                .items
                .iter_mut()
                .filter(|item| requested_ids.contains(&item.id))
            {
                if item.application.status == WalCarryoverApplicationStatus::Accepted {
                    continue;
                }
                let (updated_wal, created_entry) = WalDay::append_entry_with_conn(
                    target_wal,
                    item.content.trim().to_string(),
                    schedule_compilation,
                    conn,
                )?;
                target_wal = updated_wal;
                item.application.status = WalCarryoverApplicationStatus::Accepted;
                item.application.target_entry_id = Some(created_entry.id);
            }
            diesel::update(wal_projections::table.filter(wal_projections::id.eq(projection.id)))
                .set(wal_projections::content.eq(Some(serde_json::to_value(content)?)))
                .execute(conn)?;
            Ok(target_wal)
        })
    }
}
