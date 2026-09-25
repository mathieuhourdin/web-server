use chrono::NaiveDateTime;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Array, BigInt, Nullable, Text, Timestamp, Uuid as SqlUuid};
use diesel::PgSortExpressionMethods;
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::post::PostStatus;
use crate::entities_v2::post_grant::PostGrant;
use crate::entities_v2::{
    error::PpdcError,
    trace::Trace,
    user::{User, UserPublicResponse},
};
use crate::schema::{journals, posts, traces, users};

use super::model::{
    Journal, JournalAudienceAccessVia, JournalAudienceDefaultSharingPolicy, JournalAudienceMember,
    JournalAudienceTrace, JournalSharingMode, JournalStatus, JournalType,
};

type JournalTuple = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    bool,
    Option<NaiveDateTime>,
    Option<Uuid>,
    String,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

#[derive(QueryableByName)]
struct JournalAudienceRow {
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    active_default_sharing_policy_id: Option<Uuid>,
    #[diesel(sql_type = BigInt)]
    accessible_trace_count: i64,
    #[diesel(sql_type = Array<Text>)]
    access_via: Vec<String>,
    #[diesel(sql_type = BigInt)]
    total: i64,
}

#[derive(QueryableByName)]
struct JournalAudienceTraceRow {
    #[diesel(sql_type = SqlUuid)]
    trace_id: Uuid,
    #[diesel(sql_type = Array<Text>)]
    access_via: Vec<String>,
    #[diesel(sql_type = BigInt)]
    total: i64,
}

const JOURNAL_ACCESS_PATHS_CTE: &str = r#"
    WITH journal_traces AS (
        SELECT trace.id, trace.user_id
        FROM traces trace
        WHERE trace.journal_id = $1
          AND trace.trace_type = 'USER_TRACE'
          AND trace.status = 'FINALIZED'
          AND trace.is_encrypted = FALSE
    ),
    access_paths AS (
        SELECT grant_row.grantee_user_id AS user_id,
               trace.id AS trace_id,
               'direct_post_grant'::text AS access_via
        FROM journal_traces trace
        INNER JOIN posts post
            ON post.source_trace_id = trace.id
           AND post.status = 'PUBLISHED'
        INNER JOIN post_grants grant_row
            ON grant_row.post_id = post.id
           AND grant_row.status = 'ACTIVE'
           AND grant_row.grantee_user_id IS NOT NULL
           AND grant_row.grantee_scope IS NULL
        WHERE NOT EXISTS (
            SELECT 1 FROM user_blocks block
            WHERE (block.blocker_user_id = trace.user_id AND block.blocked_user_id = grant_row.grantee_user_id)
               OR (block.blocker_user_id = grant_row.grantee_user_id AND block.blocked_user_id = trace.user_id)
               OR (block.blocker_user_id = post.user_id AND block.blocked_user_id = grant_row.grantee_user_id)
               OR (block.blocker_user_id = grant_row.grantee_user_id AND block.blocked_user_id = post.user_id)
        )

        UNION ALL

        SELECT mention.mentioned_user_id AS user_id,
               trace.id AS trace_id,
               'mention'::text AS access_via
        FROM journal_traces trace
        INNER JOIN trace_mentions mention
            ON mention.trace_id = trace.id
           AND mention.removed_at IS NULL
        WHERE NOT EXISTS (
            SELECT 1 FROM user_blocks block
            WHERE (block.blocker_user_id = trace.user_id AND block.blocked_user_id = mention.mentioned_user_id)
               OR (block.blocker_user_id = mention.mentioned_user_id AND block.blocked_user_id = trace.user_id)
        )

        UNION ALL

        SELECT grant_row.grantee_user_id AS user_id,
               source.id AS trace_id,
               'reshare'::text AS access_via
        FROM journal_traces source
        INNER JOIN traces proxy
            ON proxy.linked_source_trace_id = source.id
           AND proxy.trace_type = 'LINKED_TRACE'
           AND proxy.status = 'FINALIZED'
        INNER JOIN trace_mentions mention
            ON mention.trace_id = source.id
           AND mention.mentioned_user_id = proxy.user_id
           AND mention.removed_at IS NULL
           AND mention.allows_reshare = TRUE
        INNER JOIN posts post
            ON post.source_trace_id = proxy.id
           AND post.status = 'PUBLISHED'
        INNER JOIN post_grants grant_row
            ON grant_row.post_id = post.id
           AND grant_row.status = 'ACTIVE'
           AND grant_row.grantee_user_id IS NOT NULL
           AND grant_row.grantee_scope IS NULL
        WHERE NOT EXISTS (
            SELECT 1 FROM user_blocks block
            WHERE (block.blocker_user_id = source.user_id AND block.blocked_user_id = proxy.user_id)
               OR (block.blocker_user_id = proxy.user_id AND block.blocked_user_id = source.user_id)
               OR (block.blocker_user_id = proxy.user_id AND block.blocked_user_id = grant_row.grantee_user_id)
               OR (block.blocker_user_id = grant_row.grantee_user_id AND block.blocked_user_id = proxy.user_id)
               OR (block.blocker_user_id = source.user_id AND block.blocked_user_id = grant_row.grantee_user_id)
               OR (block.blocker_user_id = grant_row.grantee_user_id AND block.blocked_user_id = source.user_id)
               OR (block.blocker_user_id = post.user_id AND block.blocked_user_id = grant_row.grantee_user_id)
               OR (block.blocker_user_id = grant_row.grantee_user_id AND block.blocked_user_id = post.user_id)
        )
    )
"#;

const JOURNAL_AUDIENCE_SQL_TAIL: &str = r#"
    ,
    grouped_access AS (
        SELECT user_id,
               COUNT(DISTINCT trace_id)::bigint AS accessible_trace_count,
               ARRAY_AGG(DISTINCT access_via ORDER BY access_via) AS access_via
        FROM access_paths
        WHERE user_id IS NOT NULL
          AND user_id <> $4
        GROUP BY user_id
    ),
    default_policy_users AS (
        SELECT policy.id AS policy_id,
               policy.grantee_user_id AS user_id
        FROM journal_sharing_policies policy
        WHERE policy.journal_id = $1
          AND policy.status = 'ACTIVE'
          AND policy.default_future_access_enabled = TRUE
          AND policy.grantee_user_id <> $4
    ),
    audience AS (
        SELECT policy.user_id,
               policy.policy_id AS active_default_sharing_policy_id,
               COALESCE(access.accessible_trace_count, 0)::bigint AS accessible_trace_count,
               COALESCE(access.access_via, ARRAY[]::text[]) AS access_via
        FROM default_policy_users policy
        LEFT JOIN grouped_access access ON access.user_id = policy.user_id

        UNION ALL

        SELECT access.user_id,
               NULL::uuid AS active_default_sharing_policy_id,
               access.accessible_trace_count,
               access.access_via
        FROM grouped_access access
        LEFT JOIN default_policy_users policy ON policy.user_id = access.user_id
        WHERE policy.user_id IS NULL
    )
    SELECT audience.user_id,
           audience.active_default_sharing_policy_id,
           audience.accessible_trace_count,
           audience.access_via,
           COUNT(*) OVER()::bigint AS total
    FROM audience
    INNER JOIN users reader ON reader.id = audience.user_id
    ORDER BY LOWER(reader.handle), audience.user_id
    LIMIT $2 OFFSET $3
"#;

const JOURNAL_AUDIENCE_TRACES_SQL_TAIL: &str = r#"
    ,
    accessible_traces AS (
        SELECT trace_id,
               ARRAY_AGG(DISTINCT access_via ORDER BY access_via) AS access_via
        FROM access_paths
        WHERE user_id = $5
          AND user_id <> $4
        GROUP BY trace_id
    )
    SELECT access.trace_id,
           access.access_via,
           COUNT(*) OVER()::bigint AS total
    FROM accessible_traces access
    INNER JOIN traces trace ON trace.id = access.trace_id
    ORDER BY trace.finalized_at DESC NULLS LAST, trace.id DESC
    LIMIT $2 OFFSET $3
"#;

fn journal_access_query(tail: &str) -> String {
    format!("{JOURNAL_ACCESS_PATHS_CTE}{tail}")
}

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
            current_draft_id,
            journal_type,
            status,
            sharing_mode,
            created_at,
            updated_at,
        ) = row;
        Journal {
            id,
            user_id,
            title,
            subtitle,
            content,
            is_encrypted,
            last_trace_at,
            current_draft_id,
            status: JournalStatus::from_db(&status),
            journal_type: JournalType::from_db(&journal_type),
            sharing_mode: JournalSharingMode::from_db(&sharing_mode),
            created_at,
            updated_at,
        }
    }
}

fn select_journal_columns() -> (
    journals::id,
    journals::user_id,
    journals::title,
    journals::subtitle,
    journals::content,
    journals::is_encrypted,
    journals::last_trace_at,
    journals::current_draft_id,
    journals::journal_type,
    journals::status,
    journals::sharing_mode,
    journals::created_at,
    journals::updated_at,
) {
    (
        journals::id,
        journals::user_id,
        journals::title,
        journals::subtitle,
        journals::content,
        journals::is_encrypted,
        journals::last_trace_at,
        journals::current_draft_id,
        journals::journal_type,
        journals::status,
        journals::sharing_mode,
        journals::created_at,
        journals::updated_at,
    )
}

impl Journal {
    /// Lists named people who either have a future-sharing default for this
    /// journal or current effective access to one of its traces. Broad
    /// `ALL_PLATFORM_USERS` grants are intentionally excluded.
    pub fn find_audience_paginated(
        journal_id: Uuid,
        owner_user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<JournalAudienceMember>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let rows = sql_query(journal_access_query(JOURNAL_AUDIENCE_SQL_TAIL))
            .bind::<SqlUuid, _>(journal_id)
            .bind::<BigInt, _>(limit)
            .bind::<BigInt, _>(offset)
            .bind::<SqlUuid, _>(owner_user_id)
            .load::<JournalAudienceRow>(&mut conn)?;
        let total = rows.first().map(|row| row.total).unwrap_or(0);
        let user_ids = rows.iter().map(|row| row.user_id).collect::<Vec<_>>();
        let users_by_id = users::table
            .filter(users::id.eq_any(&user_ids))
            .select(User::as_select())
            .load::<User>(&mut conn)?
            .into_iter()
            .map(|user| (user.id, user))
            .collect::<HashMap<_, _>>();

        let readers = rows
            .into_iter()
            .filter_map(|row| {
                let user = users_by_id.get(&row.user_id)?;
                Some(JournalAudienceMember {
                    user: UserPublicResponse::from(user),
                    active_default_sharing_policy: row
                        .active_default_sharing_policy_id
                        .map(|id| JournalAudienceDefaultSharingPolicy { id }),
                    accessible_trace_count: row.accessible_trace_count,
                    access_via: row
                        .access_via
                        .iter()
                        .filter_map(|value| JournalAudienceAccessVia::from_query_value(value))
                        .collect(),
                })
            })
            .collect();
        Ok((readers, total))
    }

    pub fn find_audience_user_traces_paginated(
        journal_id: Uuid,
        owner_user_id: Uuid,
        audience_user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<JournalAudienceTrace>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let rows = sql_query(journal_access_query(JOURNAL_AUDIENCE_TRACES_SQL_TAIL))
            .bind::<SqlUuid, _>(journal_id)
            .bind::<BigInt, _>(limit)
            .bind::<BigInt, _>(offset)
            .bind::<SqlUuid, _>(owner_user_id)
            .bind::<SqlUuid, _>(audience_user_id)
            .load::<JournalAudienceTraceRow>(&mut conn)?;
        let total = rows.first().map(|row| row.total).unwrap_or(0);
        let trace_ids = rows.iter().map(|row| row.trace_id).collect::<Vec<_>>();
        let traces_by_id = Trace::find_full_traces_by_ids(&trace_ids, pool)?
            .into_iter()
            .map(|trace| (trace.id, trace))
            .collect::<HashMap<_, _>>();
        let traces = rows
            .into_iter()
            .filter_map(|row| {
                let trace = traces_by_id.get(&row.trace_id)?.clone();
                Some(JournalAudienceTrace {
                    trace,
                    access_via: row
                        .access_via
                        .iter()
                        .filter_map(|value| JournalAudienceAccessVia::from_query_value(value))
                        .collect(),
                })
            })
            .collect();
        Ok((traces, total))
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<Journal, PpdcError> {
        let mut conn = pool.get()?;

        let row = journals::table
            .filter(journals::id.eq(id))
            .select(select_journal_columns())
            .first::<JournalTuple>(&mut conn)?;

        Ok(row.into())
    }

    pub fn find_full(id: Uuid, pool: &DbPool) -> Result<Journal, PpdcError> {
        Journal::find(id, pool)
    }

    pub fn find_for_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<Journal>, PpdcError> {
        let (items, _) = Self::find_for_user_paginated(
            user_id,
            0,
            i64::MAX / 4,
            JournalType::UserJournal,
            pool,
        )?;
        Ok(items)
    }

    pub fn find_all_owned_by(user_id: Uuid, pool: &DbPool) -> Result<Vec<Journal>, PpdcError> {
        let mut conn = pool.get()?;
        let rows = journals::table
            .filter(journals::user_id.eq(user_id))
            .select(select_journal_columns())
            .order((journals::created_at.asc(), journals::id.asc()))
            .load::<JournalTuple>(&mut conn)?;
        Ok(rows.into_iter().map(Journal::from).collect())
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
        journal_type: JournalType,
        pool: &DbPool,
    ) -> Result<(Vec<Journal>, i64), PpdcError> {
        let mut conn = pool.get()?;

        let total = journals::table
            .filter(journals::user_id.eq(user_id))
            .filter(journals::journal_type.eq(journal_type.to_db()))
            .count()
            .get_result::<i64>(&mut conn)?;

        let rows = journals::table
            .filter(journals::user_id.eq(user_id))
            .filter(journals::journal_type.eq(journal_type.to_db()))
            .select(select_journal_columns())
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
            .select(select_journal_columns())
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
        viewer_user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Journal>, i64), PpdcError> {
        let visible_post_ids = PostGrant::find_shared_post_ids_for_user(viewer_user_id, pool)?;
        if visible_post_ids.is_empty() {
            return Ok((vec![], 0));
        }

        let mut conn = pool.get()?;

        let total = journals::table
            .inner_join(traces::table.on(traces::journal_id.eq(journals::id)))
            .inner_join(posts::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(posts::id.eq_any(&visible_post_ids))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .filter(journals::is_encrypted.eq(false))
            .filter(journals::status.ne(JournalStatus::Archived.to_db()))
            .select(sql::<BigInt>("COUNT(DISTINCT journals.id)"))
            .first::<i64>(&mut conn)?;

        let rows = journals::table
            .inner_join(traces::table.on(traces::journal_id.eq(journals::id)))
            .inner_join(posts::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(posts::id.eq_any(visible_post_ids))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
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
                journals::current_draft_id,
                journals::journal_type,
                journals::status,
                journals::sharing_mode,
                journals::created_at,
                journals::updated_at,
            ))
            .select(select_journal_columns())
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

    pub fn find_recent_shared_for_owner_paginated(
        viewer_user_id: Uuid,
        journal_owner_user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Journal>, i64), PpdcError> {
        let visible_post_ids = PostGrant::find_shared_post_ids_for_user(viewer_user_id, pool)?;
        if visible_post_ids.is_empty() {
            return Ok((vec![], 0));
        }

        let mut conn = pool.get()?;
        let total = journals::table
            .inner_join(traces::table.on(traces::journal_id.eq(journals::id)))
            .inner_join(posts::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(journals::user_id.eq(journal_owner_user_id))
            .filter(posts::id.eq_any(&visible_post_ids))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .filter(journals::is_encrypted.eq(false))
            .filter(journals::status.ne(JournalStatus::Archived.to_db()))
            .select(sql::<BigInt>("COUNT(DISTINCT journals.id)"))
            .first::<i64>(&mut conn)?;

        let rows = journals::table
            .inner_join(traces::table.on(traces::journal_id.eq(journals::id)))
            .inner_join(posts::table.on(posts::source_trace_id.eq(traces::id.nullable())))
            .filter(journals::user_id.eq(journal_owner_user_id))
            .filter(posts::id.eq_any(visible_post_ids))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
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
                journals::current_draft_id,
                journals::journal_type,
                journals::status,
                journals::sharing_mode,
                journals::created_at,
                journals::updated_at,
            ))
            .select(select_journal_columns())
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
