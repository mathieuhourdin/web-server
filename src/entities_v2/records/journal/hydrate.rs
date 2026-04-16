use chrono::NaiveDateTime;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Nullable, Timestamp};
use diesel::PgSortExpressionMethods;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::entities_v2::post_grant::PostGrant;
use crate::schema::{journals, posts, traces};

use super::model::{Journal, JournalStatus, JournalType};

type JournalTuple = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    bool,
    Option<NaiveDateTime>,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

impl From<JournalTuple> for Journal {
    fn from(row: JournalTuple) -> Self {
        let (
            id,
            user_id,
            title,
            subtitle,
            content,
            is_encrypted,
            last_trace_at,
            journal_type,
            status,
            created_at,
            updated_at,
        ) = row;
        Journal {
            id,
            title,
            subtitle,
            content,
            is_encrypted,
            last_trace_at,
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
        let mut conn = pool.get()?;

        let row = journals::table
            .filter(journals::id.eq(id))
            .select((
                journals::id,
                journals::user_id,
                journals::title,
                journals::subtitle,
                journals::content,
                journals::is_encrypted,
                journals::last_trace_at,
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
        let (items, _) = Self::find_for_user_paginated(user_id, 0, i64::MAX / 4, pool)?;
        Ok(items)
    }

    pub fn count_for_user(user_id: Uuid, pool: &DbPool) -> Result<i64, PpdcError> {
        let mut conn = pool.get()?;

        let total = journals::table
            .filter(journals::user_id.eq(user_id))
            .count()
            .get_result::<i64>(&mut conn)?;

        Ok(total)
    }

    pub fn find_for_user_paginated(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Journal>, i64), PpdcError> {
        let mut conn = pool.get()?;

        let total = journals::table
            .filter(journals::user_id.eq(user_id))
            .count()
            .get_result::<i64>(&mut conn)?;

        let rows = journals::table
            .filter(journals::user_id.eq(user_id))
            .select((
                journals::id,
                journals::user_id,
                journals::title,
                journals::subtitle,
                journals::content,
                journals::is_encrypted,
                journals::last_trace_at,
                journals::journal_type,
                journals::status,
                journals::created_at,
                journals::updated_at,
            ))
            .order((
                journals::last_trace_at.desc().nulls_last(),
                journals::updated_at.desc(),
            ))
            .offset(offset)
            .limit(limit)
            .load::<JournalTuple>(&mut conn)?;

        Ok((rows.into_iter().map(Journal::from).collect(), total))
    }

    pub fn find_many(ids: Vec<Uuid>, pool: &DbPool) -> Result<Vec<Journal>, PpdcError> {
        let (items, _) = Self::find_many_paginated(ids, 0, i64::MAX / 4, false, pool)?;
        Ok(items)
    }

    pub fn find_many_paginated(
        ids: Vec<Uuid>,
        offset: i64,
        limit: i64,
        visible_only: bool,
        pool: &DbPool,
    ) -> Result<(Vec<Journal>, i64), PpdcError> {
        if ids.is_empty() {
            return Ok((vec![], 0));
        }

        let mut conn = pool.get()?;

        let total = {
            let mut count_query = journals::table
                .filter(journals::id.eq_any(&ids))
                .into_boxed();

            if visible_only {
                count_query = count_query
                    .filter(journals::is_encrypted.eq(false))
                    .filter(journals::status.ne(JournalStatus::Archived.to_db()));
            }

            count_query.count().get_result::<i64>(&mut conn)?
        };

        let mut items_query = journals::table
            .filter(journals::id.eq_any(ids))
            .into_boxed();

        if visible_only {
            items_query = items_query
                .filter(journals::is_encrypted.eq(false))
                .filter(journals::status.ne(JournalStatus::Archived.to_db()));
        }

        let rows = items_query
            .select((
                journals::id,
                journals::user_id,
                journals::title,
                journals::subtitle,
                journals::content,
                journals::is_encrypted,
                journals::last_trace_at,
                journals::journal_type,
                journals::status,
                journals::created_at,
                journals::updated_at,
            ))
            .order((
                journals::last_trace_at.desc().nulls_last(),
                journals::updated_at.desc(),
            ))
            .offset(offset)
            .limit(limit)
            .load::<JournalTuple>(&mut conn)?;

        Ok((rows.into_iter().map(Journal::from).collect(), total))
    }

    pub fn find_recent_shared_for_user_paginated(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Journal>, i64), PpdcError> {
        let visible_post_ids = PostGrant::find_shared_post_ids_for_user(user_id, pool)?;
        if visible_post_ids.is_empty() {
            return Ok((vec![], 0));
        }

        let mut conn = pool.get()?;

        let total = journals::table
            .inner_join(traces::table.on(traces::journal_id.eq(journals::id)))
            .inner_join(posts::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(posts::id.eq_any(&visible_post_ids))
            .filter(journals::is_encrypted.eq(false))
            .filter(journals::status.ne(JournalStatus::Archived.to_db()))
            .select(sql::<BigInt>("COUNT(DISTINCT journals.id)"))
            .first::<i64>(&mut conn)?;

        let rows = journals::table
            .inner_join(traces::table.on(traces::journal_id.eq(journals::id)))
            .inner_join(posts::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(posts::id.eq_any(visible_post_ids))
            .filter(journals::is_encrypted.eq(false))
            .filter(journals::status.ne(JournalStatus::Archived.to_db()))
            .group_by((
                journals::id,
                journals::user_id,
                journals::title,
                journals::subtitle,
                journals::content,
                journals::is_encrypted,
                journals::last_trace_at,
                journals::journal_type,
                journals::status,
                journals::created_at,
                journals::updated_at,
            ))
            .select((
                journals::id,
                journals::user_id,
                journals::title,
                journals::subtitle,
                journals::content,
                journals::is_encrypted,
                journals::last_trace_at,
                journals::journal_type,
                journals::status,
                journals::created_at,
                journals::updated_at,
            ))
            .order(
                sql::<Nullable<Timestamp>>(
                    "MAX(COALESCE(posts.publishing_date, posts.updated_at, posts.created_at))",
                )
                .desc()
                .nulls_last(),
            )
            .then_order_by(journals::updated_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<JournalTuple>(&mut conn)?;

        Ok((rows.into_iter().map(Journal::from).collect(), total))
    }
}
