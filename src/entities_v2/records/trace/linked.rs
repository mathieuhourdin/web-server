use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{
    Array, BigInt, Bool, Int4, Nullable, Text, Timestamp, Timestamptz, Uuid as SqlUuid,
};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    journal::{Journal, JournalStatus, JournalType},
    trace_mention::TraceMention,
};

use super::{
    model::{Trace, TraceListItem},
    persist::recalculate_journal_last_trace_at,
    TraceSharingSensitivity, TraceStatus, TraceType,
};

#[derive(Debug, Clone, Serialize, QueryableByName)]
pub struct LinkedTraceResponse {
    #[diesel(sql_type = SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    pub journal_id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    pub source_trace_id: Uuid,
    #[diesel(sql_type = Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    pub updated_at: NaiveDateTime,
}

#[derive(QueryableByName)]
struct OptionalJournalIdRow {
    #[diesel(sql_type = Nullable<SqlUuid>)]
    journal_id: Option<Uuid>,
}

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    #[diesel(column_name = id)]
    _id: Uuid,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct BoolRow {
    #[diesel(sql_type = Bool)]
    value: bool,
}

#[derive(QueryableByName)]
struct JournalTraceListRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    source_trace_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    post_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<Int4>)]
    version_integer: Option<i32>,
    #[diesel(sql_type = SqlUuid)]
    journal_id: Uuid,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    subtitle: String,
    #[diesel(sql_type = Text)]
    content: String,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    derived_from_trace_id: Option<Uuid>,
    #[diesel(sql_type = Bool)]
    is_encrypted: bool,
    #[diesel(sql_type = Nullable<Text>)]
    encryption_metadata: Option<String>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    content_image_asset_id: Option<Uuid>,
    #[diesel(sql_type = Text)]
    sharing_sensitivity: String,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    timeout_start_at: Option<DateTime<Utc>>,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    timeout_at: Option<DateTime<Utc>>,
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type = Text)]
    trace_type: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Timestamp)]
    start_writing_at: NaiveDateTime,
    #[diesel(sql_type = Nullable<Timestamp>)]
    finalized_at: Option<NaiveDateTime>,
    #[diesel(sql_type = Timestamp)]
    interaction_date: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
}

const JOURNAL_CANDIDATES_SQL: &str = r#"
    SELECT
        owned.id AS item_id,
        owned.id AS source_id,
        NULL::uuid AS linked_source_id,
        owned.journal_id,
        owned.trace_type AS item_type,
        owned.status AS item_status,
        owned.version_integer AS item_version,
        owned.created_at AS item_created_at,
        owned.updated_at AS item_updated_at
    FROM traces owned
    WHERE owned.journal_id = $1
      AND owned.user_id = $2
      AND owned.trace_type = 'USER_TRACE'
      AND owned.status = $3

    UNION ALL

    SELECT
        proxy.id AS item_id,
        source.id AS source_id,
        source.id AS linked_source_id,
        proxy.journal_id,
        proxy.trace_type AS item_type,
        proxy.status AS item_status,
        proxy.version_integer AS item_version,
        proxy.created_at AS item_created_at,
        proxy.updated_at AS item_updated_at
    FROM traces proxy
    INNER JOIN traces source ON source.id = proxy.linked_source_trace_id
    WHERE proxy.journal_id = $1
      AND proxy.user_id = $2
      AND proxy.trace_type = 'LINKED_TRACE'
      AND proxy.status = $3
      AND source.trace_type = 'USER_TRACE'
      AND source.status = 'FINALIZED'
      AND source.is_encrypted = FALSE
      AND NOT EXISTS (
          SELECT 1
          FROM user_blocks block
          WHERE (block.blocker_user_id = source.user_id AND block.blocked_user_id = $2)
             OR (block.blocker_user_id = $2 AND block.blocked_user_id = source.user_id)
      )
      AND (
          EXISTS (
              SELECT 1
              FROM trace_mentions mention
              WHERE mention.trace_id = source.id
                AND mention.mentioned_user_id = $2
                AND mention.removed_at IS NULL
          )
          OR EXISTS (
              SELECT 1
              FROM posts source_post
              INNER JOIN post_grants grant_row ON grant_row.post_id = source_post.id
              WHERE source_post.source_trace_id = source.id
                AND source_post.status = 'PUBLISHED'
                AND grant_row.status = 'ACTIVE'
                AND (
                    grant_row.grantee_user_id = $2
                    OR (
                        grant_row.grantee_scope = 'ALL_PLATFORM_USERS'
                        AND EXISTS (
                            SELECT 1 FROM users viewer
                            WHERE viewer.id = $2
                              AND viewer.is_platform_user = TRUE
                              AND viewer.principal_type = 'HUMAN'
                        )
                    )
                )
          )
      )
"#;

impl LinkedTraceResponse {
    pub fn reshare_is_active(
        linked_trace_id: Uuid,
        resharer_user_id: Uuid,
        viewer_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let mut conn = pool.get()?;
        Ok(sql_query(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM traces proxy
                INNER JOIN traces source ON source.id = proxy.linked_source_trace_id
                INNER JOIN trace_mentions mention
                    ON mention.trace_id = source.id
                   AND mention.mentioned_user_id = proxy.user_id
                   AND mention.removed_at IS NULL
                   AND mention.allows_reshare = TRUE
                WHERE proxy.id = $1
                  AND proxy.user_id = $2
                  AND proxy.trace_type = 'LINKED_TRACE'
                  AND proxy.status = 'FINALIZED'
                  AND source.trace_type = 'USER_TRACE'
                  AND source.status = 'FINALIZED'
                  AND source.is_encrypted = FALSE
                  AND NOT EXISTS (
                      SELECT 1 FROM user_blocks block
                      WHERE (block.blocker_user_id = source.user_id AND block.blocked_user_id = $2)
                         OR (block.blocker_user_id = $2 AND block.blocked_user_id = source.user_id)
                         OR (block.blocker_user_id = source.user_id AND block.blocked_user_id = $3)
                         OR (block.blocker_user_id = $3 AND block.blocked_user_id = source.user_id)
                  )
            ) AS value
            "#,
        )
        .bind::<SqlUuid, _>(linked_trace_id)
        .bind::<SqlUuid, _>(resharer_user_id)
        .bind::<SqlUuid, _>(viewer_user_id)
        .get_result::<BoolRow>(&mut conn)?
        .value)
    }

    pub fn filter_eligible_reshare_post_ids(
        post_ids: &[Uuid],
        viewer_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        if post_ids.is_empty() {
            return Ok(Vec::new());
        }
        #[derive(QueryableByName)]
        struct PostIdRow {
            #[diesel(sql_type = SqlUuid)]
            id: Uuid,
        }
        let mut conn = pool.get()?;
        Ok(sql_query(
            r#"
            SELECT post.id
            FROM posts post
            LEFT JOIN traces proxy ON proxy.id = post.source_trace_id
            LEFT JOIN traces source ON source.id = proxy.linked_source_trace_id
            WHERE post.id = ANY($1)
              AND (
                  proxy.trace_type IS DISTINCT FROM 'LINKED_TRACE'
                  OR (
                      proxy.user_id = post.user_id
                      AND proxy.status = 'FINALIZED'
                      AND source.trace_type = 'USER_TRACE'
                      AND source.status = 'FINALIZED'
                      AND source.is_encrypted = FALSE
                      AND EXISTS (
                          SELECT 1 FROM trace_mentions mention
                          WHERE mention.trace_id = source.id
                            AND mention.mentioned_user_id = proxy.user_id
                            AND mention.removed_at IS NULL
                            AND mention.allows_reshare = TRUE
                      )
                      AND NOT EXISTS (
                          SELECT 1 FROM user_blocks block
                          WHERE (block.blocker_user_id = source.user_id AND block.blocked_user_id = proxy.user_id)
                             OR (block.blocker_user_id = proxy.user_id AND block.blocked_user_id = source.user_id)
                             OR (block.blocker_user_id = source.user_id AND block.blocked_user_id = $2)
                             OR (block.blocker_user_id = $2 AND block.blocked_user_id = source.user_id)
                      )
                  )
              )
            "#,
        )
        .bind::<Array<SqlUuid>, _>(post_ids)
        .bind::<SqlUuid, _>(viewer_user_id)
        .load::<PostIdRow>(&mut conn)?
        .into_iter()
        .map(|row| row.id)
        .collect())
    }

    pub fn filter_eligible_reshare_viewers(
        linked_trace_id: Uuid,
        resharer_user_id: Uuid,
        viewer_user_ids: &[Uuid],
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        if viewer_user_ids.is_empty() {
            return Ok(Vec::new());
        }
        #[derive(QueryableByName)]
        struct UserIdRow {
            #[diesel(sql_type = SqlUuid)]
            id: Uuid,
        }
        let mut conn = pool.get()?;
        Ok(sql_query(
            r#"
            SELECT viewer.id
            FROM unnest($3::uuid[]) AS viewer(id)
            WHERE EXISTS (
                SELECT 1
                FROM traces proxy
                INNER JOIN traces source ON source.id = proxy.linked_source_trace_id
                INNER JOIN trace_mentions mention
                    ON mention.trace_id = source.id
                   AND mention.mentioned_user_id = proxy.user_id
                   AND mention.removed_at IS NULL
                   AND mention.allows_reshare = TRUE
                WHERE proxy.id = $1
                  AND proxy.user_id = $2
                  AND proxy.trace_type = 'LINKED_TRACE'
                  AND proxy.status = 'FINALIZED'
                  AND source.trace_type = 'USER_TRACE'
                  AND source.status = 'FINALIZED'
                  AND source.is_encrypted = FALSE
                  AND NOT EXISTS (
                      SELECT 1 FROM user_blocks block
                      WHERE (block.blocker_user_id = source.user_id AND block.blocked_user_id = $2)
                         OR (block.blocker_user_id = $2 AND block.blocked_user_id = source.user_id)
                         OR (block.blocker_user_id = source.user_id AND block.blocked_user_id = viewer.id)
                         OR (block.blocker_user_id = viewer.id AND block.blocked_user_id = source.user_id)
                  )
            )
            "#,
        )
        .bind::<SqlUuid, _>(linked_trace_id)
        .bind::<SqlUuid, _>(resharer_user_id)
        .bind::<Array<SqlUuid>, _>(viewer_user_ids)
        .load::<UserIdRow>(&mut conn)?
        .into_iter()
        .map(|row| row.id)
        .collect())
    }

    pub fn source_is_readable_via_reshare(
        source_trace_id: Uuid,
        viewer_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let mut conn = pool.get()?;
        Ok(sql_query(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM traces source
                INNER JOIN traces proxy ON proxy.linked_source_trace_id = source.id
                INNER JOIN posts post
                    ON post.source_trace_id = proxy.id
                   AND post.status = 'PUBLISHED'
                INNER JOIN post_grants grant_row
                    ON grant_row.post_id = post.id
                   AND grant_row.status = 'ACTIVE'
                INNER JOIN trace_mentions mention
                    ON mention.trace_id = source.id
                   AND mention.mentioned_user_id = proxy.user_id
                   AND mention.removed_at IS NULL
                   AND mention.allows_reshare = TRUE
                WHERE source.id = $1
                  AND source.trace_type = 'USER_TRACE'
                  AND source.status = 'FINALIZED'
                  AND source.is_encrypted = FALSE
                  AND proxy.trace_type = 'LINKED_TRACE'
                  AND proxy.status = 'FINALIZED'
                  AND (
                      grant_row.grantee_user_id = $2
                      OR (
                          grant_row.grantee_scope = 'ALL_PLATFORM_USERS'
                          AND EXISTS (
                              SELECT 1 FROM users viewer
                              WHERE viewer.id = $2
                                AND viewer.is_platform_user = TRUE
                                AND viewer.principal_type = 'HUMAN'
                          )
                      )
                  )
                  AND NOT EXISTS (
                      SELECT 1 FROM user_blocks block
                      WHERE (block.blocker_user_id = proxy.user_id AND block.blocked_user_id = $2)
                         OR (block.blocker_user_id = $2 AND block.blocked_user_id = proxy.user_id)
                         OR (block.blocker_user_id = source.user_id AND block.blocked_user_id = proxy.user_id)
                         OR (block.blocker_user_id = proxy.user_id AND block.blocked_user_id = source.user_id)
                         OR (block.blocker_user_id = source.user_id AND block.blocked_user_id = $2)
                         OR (block.blocker_user_id = $2 AND block.blocked_user_id = source.user_id)
                  )
            ) AS value
            "#,
        )
        .bind::<SqlUuid, _>(source_trace_id)
        .bind::<SqlUuid, _>(viewer_user_id)
        .get_result::<BoolRow>(&mut conn)?
        .value)
    }

    pub fn create_or_move(
        journal_id: Uuid,
        source_trace_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Self, PpdcError> {
        let journal = Journal::find_full(journal_id, pool)?;
        if journal.user_id != user_id {
            return Err(PpdcError::unauthorized());
        }
        if journal.status != JournalStatus::Active
            || journal.journal_type != JournalType::UserJournal
            || journal.is_encrypted
        {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Linked traces require an active, unencrypted user journal".to_string(),
            ));
        }

        let source = Trace::find_full_trace(source_trace_id, pool)?;
        if source.user_id == user_id
            || source.trace_type != TraceType::UserTrace
            || source.status != TraceStatus::Finalized
            || source.is_encrypted
        {
            return Err(PpdcError::new(
                422,
                ErrorType::ApiError,
                "Trace cannot be linked into this journal".to_string(),
            ));
        }
        if !TraceMention::active_mention_exists(source.id, user_id, pool)? {
            return Err(PpdcError::new(
                403,
                ErrorType::ApiError,
                "Only an actively mentioned user can link this trace".to_string(),
            ));
        }
        if !source.user_can_read(user_id, pool)? {
            return Err(PpdcError::unauthorized());
        }

        let mut conn = pool.get()?;
        conn.transaction::<Self, PpdcError, _>(|conn| {
            // Serialize the one-per-user/source upsert so a concurrent move can always observe
            // and recalculate the journal selected by the preceding transaction.
            sql_query("SELECT id FROM traces WHERE id = $1 FOR UPDATE")
                .bind::<SqlUuid, _>(source_trace_id)
                .get_result::<IdRow>(conn)?;
            let previous = sql_query(
                "SELECT journal_id
                 FROM traces
                 WHERE user_id = $1
                   AND linked_source_trace_id = $2
                   AND trace_type = 'LINKED_TRACE'
                 FOR UPDATE",
            )
            .bind::<SqlUuid, _>(user_id)
            .bind::<SqlUuid, _>(source_trace_id)
            .get_result::<OptionalJournalIdRow>(conn)
            .optional()?;

            let now = Utc::now().naive_utc();
            let linked = sql_query(
                "INSERT INTO traces (
                    id, user_id, journal_id, title, subtitle, content, interaction_date,
                    trace_type, status, is_encrypted, encryption_metadata,
                    start_writing_at, finalized_at, content_image_asset_id,
                    timeout_at, timeout_start_at, sharing_sensitivity,
                    derived_from_trace_id, linked_source_trace_id, is_blank, version_integer,
                    created_at, updated_at
                 ) VALUES (
                    $1, $2, $3, '', '', '', $4,
                    'LINKED_TRACE', 'FINALIZED', FALSE, NULL,
                    $5, $5, NULL,
                    NULL, NULL, 'NORMAL',
                    NULL, $6, FALSE, 0,
                    $5, $5
                 )
                 ON CONFLICT (user_id, linked_source_trace_id)
                     WHERE trace_type = 'LINKED_TRACE'
                 DO UPDATE SET
                    journal_id = EXCLUDED.journal_id,
                    status = 'FINALIZED',
                    interaction_date = EXCLUDED.interaction_date,
                    version_integer = traces.version_integer + 1,
                    updated_at = NOW()
                 RETURNING id, journal_id, linked_source_trace_id AS source_trace_id,
                           created_at, updated_at",
            )
            .bind::<SqlUuid, _>(Uuid::new_v4())
            .bind::<SqlUuid, _>(user_id)
            .bind::<SqlUuid, _>(journal_id)
            .bind::<Timestamp, _>(source.interaction_date)
            .bind::<Timestamp, _>(now)
            .bind::<SqlUuid, _>(source_trace_id)
            .get_result::<Self>(conn)?;

            if let Some(previous_journal_id) = previous.and_then(|row| row.journal_id) {
                if previous_journal_id != journal_id {
                    let has_post = sql_query(
                        "SELECT EXISTS (
                            SELECT 1
                            FROM traces proxy
                            INNER JOIN posts post ON post.source_trace_id = proxy.id
                            WHERE proxy.user_id = $1
                              AND proxy.linked_source_trace_id = $2
                              AND proxy.trace_type = 'LINKED_TRACE'
                        ) AS value",
                    )
                    .bind::<SqlUuid, _>(user_id)
                    .bind::<SqlUuid, _>(source_trace_id)
                    .get_result::<BoolRow>(conn)?
                    .value;
                    if has_post {
                        return Err(PpdcError::new(
                            409,
                            ErrorType::ApiError,
                            "A linked trace with a staged or published post cannot be moved between journals"
                                .to_string(),
                        ));
                    }
                    recalculate_journal_last_trace_at(conn, previous_journal_id)?;
                }
            }
            recalculate_journal_last_trace_at(conn, journal_id)?;
            Ok(linked)
        })
    }

    pub fn delete(
        journal_id: Uuid,
        source_trace_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let journal = Journal::find_full(journal_id, pool)?;
        if journal.user_id != user_id {
            return Err(PpdcError::unauthorized());
        }
        let mut conn = pool.get()?;
        conn.transaction::<bool, PpdcError, _>(|conn| {
            sql_query("SELECT id FROM traces WHERE id = $1 FOR UPDATE")
                .bind::<SqlUuid, _>(source_trace_id)
                .get_result::<IdRow>(conn)
                .optional()?;
            sql_query(
                "UPDATE posts
                 SET status = 'ARCHIVED', updated_at = NOW()
                 WHERE source_trace_id IN (
                     SELECT id FROM traces
                     WHERE journal_id = $1
                       AND user_id = $2
                       AND linked_source_trace_id = $3
                       AND trace_type = 'LINKED_TRACE'
                 )",
            )
            .bind::<SqlUuid, _>(journal_id)
            .bind::<SqlUuid, _>(user_id)
            .bind::<SqlUuid, _>(source_trace_id)
            .execute(conn)?;

            let deleted = sql_query(
                "DELETE FROM traces
                 WHERE journal_id = $1
                   AND user_id = $2
                   AND linked_source_trace_id = $3
                   AND trace_type = 'LINKED_TRACE'
                 RETURNING id",
            )
            .bind::<SqlUuid, _>(journal_id)
            .bind::<SqlUuid, _>(user_id)
            .bind::<SqlUuid, _>(source_trace_id)
            .get_result::<IdRow>(conn)
            .optional()?
            .is_some();
            if deleted {
                recalculate_journal_last_trace_at(conn, journal_id)?;
            }
            Ok(deleted)
        })
    }

    pub fn find_source_trace_id(
        linked_trace_id: Uuid,
        owner_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Uuid>, PpdcError> {
        use crate::schema::traces;
        let mut conn = pool.get()?;
        Ok(traces::table
            .filter(traces::id.eq(linked_trace_id))
            .filter(traces::user_id.eq(owner_user_id))
            .filter(traces::trace_type.eq(TraceType::LinkedTrace.to_db()))
            .select(traces::linked_source_trace_id)
            .first::<Option<Uuid>>(&mut conn)
            .optional()?
            .flatten())
    }

    pub fn find_source_trace_id_unscoped(
        linked_trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Option<Uuid>, PpdcError> {
        use crate::schema::traces;
        let mut conn = pool.get()?;
        Ok(traces::table
            .filter(traces::id.eq(linked_trace_id))
            .filter(traces::trace_type.eq(TraceType::LinkedTrace.to_db()))
            .select(traces::linked_source_trace_id)
            .first::<Option<Uuid>>(&mut conn)
            .optional()?
            .flatten())
    }

    pub fn find_journal_items_paginated(
        journal_id: Uuid,
        viewer_user_id: Uuid,
        offset: i64,
        limit: i64,
        sharing_sensitivity: Option<TraceSharingSensitivity>,
        status: TraceStatus,
        seen: Option<bool>,
        pool: &DbPool,
    ) -> Result<(Vec<TraceListItem>, i64), PpdcError> {
        let sharing = sharing_sensitivity.map(|value| value.to_db().to_string());
        let mut conn = pool.get()?;
        let count_sql = format!(
            "WITH candidates AS ({JOURNAL_CANDIDATES_SQL})
             SELECT COUNT(*)::bigint AS count
             FROM candidates c
             INNER JOIN traces source ON source.id = c.source_id
             WHERE ($4::text IS NULL OR source.sharing_sensitivity = $4)
               AND ($5::boolean IS NULL OR $5 = EXISTS (
                   SELECT 1
                   FROM posts seen_post
                   INNER JOIN user_post_states seen_state ON seen_state.post_id = seen_post.id
                   WHERE seen_post.source_trace_id = source.id
                     AND seen_state.user_id = $2
               ))"
        );
        let total = sql_query(count_sql)
            .bind::<SqlUuid, _>(journal_id)
            .bind::<SqlUuid, _>(viewer_user_id)
            .bind::<Text, _>(status.to_db())
            .bind::<Nullable<Text>, _>(sharing.clone())
            .bind::<Nullable<Bool>, _>(seen)
            .get_result::<CountRow>(&mut conn)?
            .count;

        let page_sql = format!(
            "WITH candidates AS ({JOURNAL_CANDIDATES_SQL}),
             eligible AS (
                 SELECT c.*
                 FROM candidates c
                 INNER JOIN traces source ON source.id = c.source_id
                 WHERE ($4::text IS NULL OR source.sharing_sensitivity = $4)
                   AND ($5::boolean IS NULL OR $5 = EXISTS (
                       SELECT 1
                       FROM posts seen_post
                       INNER JOIN user_post_states seen_state ON seen_state.post_id = seen_post.id
                       WHERE seen_post.source_trace_id = source.id
                         AND seen_state.user_id = $2
                   ))
             ),
             page AS (
                 SELECT e.*
                 FROM eligible e
                 INNER JOIN traces source ON source.id = e.source_id
                 ORDER BY source.finalized_at DESC NULLS LAST,
                          source.interaction_date DESC,
                          source.created_at DESC
                 OFFSET $6 LIMIT $7
             )
             SELECT
                 page.item_id AS id,
                 page.linked_source_id AS source_trace_id,
                 source_post.id AS post_id,
                 CASE
                     WHEN page.linked_source_id IS NULL THEN page.item_version
                     ELSE NULL
                 END AS version_integer,
                 page.journal_id,
                 source.title,
                 source.subtitle,
                 source.content,
                 source.derived_from_trace_id,
                 source.is_encrypted,
                 source.encryption_metadata::text AS encryption_metadata,
                 source.content_image_asset_id,
                 source.sharing_sensitivity,
                 source.timeout_start_at,
                 source.timeout_at,
                 source.user_id,
                 page.item_type AS trace_type,
                 page.item_status AS status,
                 source.start_writing_at,
                 source.finalized_at,
                 source.interaction_date,
                 page.item_created_at AS created_at,
                 page.item_updated_at AS updated_at
             FROM page
             INNER JOIN traces source ON source.id = page.source_id
             LEFT JOIN posts source_post
                    ON source_post.source_trace_id = source.id
                   AND source_post.status = 'PUBLISHED'
             ORDER BY source.finalized_at DESC NULLS LAST,
                      source.interaction_date DESC,
                      source.created_at DESC"
        );
        let rows = sql_query(page_sql)
            .bind::<SqlUuid, _>(journal_id)
            .bind::<SqlUuid, _>(viewer_user_id)
            .bind::<Text, _>(status.to_db())
            .bind::<Nullable<Text>, _>(sharing)
            .bind::<Nullable<Bool>, _>(seen)
            .bind::<BigInt, _>(offset)
            .bind::<BigInt, _>(limit)
            .load::<JournalTraceListRow>(&mut conn)?;

        Ok((rows.into_iter().map(TraceListItem::from).collect(), total))
    }

    pub fn find_journal_item_rank(
        journal_id: Uuid,
        viewer_user_id: Uuid,
        target_item_id: Uuid,
        sharing_sensitivity: Option<TraceSharingSensitivity>,
        status: TraceStatus,
        seen: Option<bool>,
        pool: &DbPool,
    ) -> Result<Option<i64>, PpdcError> {
        let sharing = sharing_sensitivity.map(|value| value.to_db().to_string());
        let rank_sql = format!(
            "WITH candidates AS ({JOURNAL_CANDIDATES_SQL}),
             eligible AS (
                 SELECT c.*, source.finalized_at, source.interaction_date,
                        source.created_at AS source_created_at
                 FROM candidates c
                 INNER JOIN traces source ON source.id = c.source_id
                 WHERE ($4::text IS NULL OR source.sharing_sensitivity = $4)
                   AND ($5::boolean IS NULL OR $5 = EXISTS (
                       SELECT 1
                       FROM posts seen_post
                       INNER JOIN user_post_states seen_state ON seen_state.post_id = seen_post.id
                       WHERE seen_post.source_trace_id = source.id
                         AND seen_state.user_id = $2
                   ))
             ),
             ranked AS (
                 SELECT item_id,
                        ROW_NUMBER() OVER (
                            ORDER BY finalized_at DESC NULLS LAST,
                                     interaction_date DESC,
                                     source_created_at DESC
                        )::bigint AS count
                 FROM eligible
             )
             SELECT count FROM ranked WHERE item_id = $6"
        );
        let mut conn = pool.get()?;
        Ok(sql_query(rank_sql)
            .bind::<SqlUuid, _>(journal_id)
            .bind::<SqlUuid, _>(viewer_user_id)
            .bind::<Text, _>(status.to_db())
            .bind::<Nullable<Text>, _>(sharing)
            .bind::<Nullable<Bool>, _>(seen)
            .bind::<SqlUuid, _>(target_item_id)
            .get_result::<CountRow>(&mut conn)
            .optional()?
            .map(|row| row.count))
    }
}

impl From<JournalTraceListRow> for TraceListItem {
    fn from(row: JournalTraceListRow) -> Self {
        Self {
            id: row.id,
            source_trace_id: row.source_trace_id,
            post_id: row.post_id,
            version_integer: row.version_integer,
            journal_id: row.journal_id,
            title: row.title,
            subtitle: Some(row.subtitle),
            content: row.content,
            derived_from_trace_id: row.derived_from_trace_id,
            is_encrypted: Some(row.is_encrypted),
            encryption_metadata: row
                .encryption_metadata
                .and_then(|json| serde_json::from_str::<Value>(&json).ok()),
            content_image_asset_id: row.content_image_asset_id,
            content_image: None,
            sharing_sensitivity: Some(TraceSharingSensitivity::from_db(&row.sharing_sensitivity)),
            timeout_start_at: row.timeout_start_at,
            timeout_at: row.timeout_at,
            user_id: Some(row.user_id),
            trace_type: Some(TraceType::from_db(&row.trace_type)),
            status: Some(TraceStatus::from_db(&row.status)),
            start_writing_at: Some(row.start_writing_at),
            finalized_at: row.finalized_at,
            seen: false,
            last_seen_at: None,
            seen_by_preview: None,
            interaction_date: row.interaction_date,
            created_at: row.created_at,
            updated_at: row.updated_at,
            mentions: Vec::new(),
        }
    }
}
