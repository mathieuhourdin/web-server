use chrono::{NaiveDate, NaiveDateTime, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::wal_days;

use super::model::{NewWalDay, WalCompilationViews, WalDay};

impl WalDay {
    fn get_or_create_with_conn(
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

    pub fn append(
        user_id: Uuid,
        local_date: NaiveDate,
        entry: String,
        pool: &DbPool,
    ) -> Result<Self, PpdcError> {
        let mut conn = pool.get()?;
        conn.transaction::<Self, PpdcError, _>(|conn| {
            Self::get_or_create_with_conn(user_id, local_date, conn)?;
            let current = wal_days::table
                .filter(wal_days::user_id.eq(user_id))
                .filter(wal_days::local_date.eq(local_date))
                .for_update()
                .select(WalDay::as_select())
                .first::<WalDay>(conn)?;
            let input = if current.input.is_empty() || current.input.ends_with('\n') {
                format!("{}{}", current.input, entry)
            } else {
                format!("{}\n{}", current.input, entry)
            };

            Ok(
                diesel::update(wal_days::table.filter(wal_days::id.eq(current.id)))
                    .set((
                        wal_days::input.eq(input),
                        wal_days::compiled_operational.eq::<Option<String>>(None),
                        wal_days::compiled_thematic.eq::<Option<String>>(None),
                        wal_days::compiled_at.eq::<Option<NaiveDateTime>>(None),
                        wal_days::updated_at.eq(diesel::dsl::now),
                    ))
                    .returning(WalDay::as_returning())
                    .get_result(conn)?,
            )
        })
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

    pub fn save_compilations_if_unchanged(
        &self,
        operational_content: String,
        thematic_content: String,
        pool: &DbPool,
    ) -> Result<WalCompilationViews, PpdcError> {
        let compiled_at = Utc::now().naive_utc();
        let mut conn = pool.get()?;
        let row = diesel::update(
            wal_days::table
                .filter(wal_days::id.eq(self.id))
                .filter(wal_days::user_id.eq(self.user_id))
                .filter(wal_days::input.eq(&self.input))
                .filter(wal_days::updated_at.eq(self.updated_at)),
        )
        .set((
            wal_days::compiled_operational.eq(Some(operational_content)),
            wal_days::compiled_thematic.eq(Some(thematic_content)),
            wal_days::compiled_at.eq(Some(compiled_at)),
            wal_days::updated_at.eq(diesel::dsl::now),
        ))
        .returning(WalDay::as_returning())
        .get_result::<WalDay>(&mut conn)
        .optional()?;

        let Some(row) = row else {
            return Err(PpdcError::new(
                409,
                ErrorType::ApiError,
                "WAL content changed while compiling; retry the compilation".to_string(),
            ));
        };
        Ok(row.compilation_views())
    }
}
