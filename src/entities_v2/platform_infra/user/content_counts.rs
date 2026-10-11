use axum::{
    extract::{Extension, Path},
    Json,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Nullable, Timestamp, Uuid as SqlUuid},
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    db::DbPool,
    entities_v2::{
        error::PpdcError, records::document::hydrate::readable_document_predicate,
        session::Session, user::User,
    },
};

#[derive(Serialize)]
pub struct UserContentCounts {
    pub traces_count: i64,
    pub albums_count: i64,
    pub documents_count: i64,
    pub oldest_trace_finalized_at: Option<DateTime<Utc>>,
}

#[derive(QueryableByName)]
struct CountsRow {
    #[diesel(sql_type = BigInt)]
    traces_count: i64,
    #[diesel(sql_type = BigInt)]
    albums_count: i64,
    #[diesel(sql_type = BigInt)]
    documents_count: i64,
    #[diesel(sql_type = Nullable<Timestamp>)]
    oldest_trace_finalized_at: Option<NaiveDateTime>,
}

pub async fn get_user_content_counts_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<UserContentCounts>, PpdcError> {
    let viewer = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    User::find(&user_id, &pool)?;
    let mut conn = pool.get()?;
    // One snapshot for all aggregates. EXISTS avoids duplicate counts when
    // multiple posts or attachments make the same source readable.
    let query = format!(
        "WITH readable_traces AS (
            SELECT t.finalized_at FROM traces t
            WHERE t.user_id = $1 AND t.trace_type = 'USER_TRACE' AND t.status = 'FINALIZED'
              AND (t.user_id = $2 OR (
                NOT t.is_encrypted AND NOT EXISTS (
                    SELECT 1 FROM user_blocks b WHERE
                      (b.blocker_user_id = t.user_id AND b.blocked_user_id = $2) OR
                      (b.blocked_user_id = t.user_id AND b.blocker_user_id = $2)
                ) AND (
                    EXISTS (SELECT 1 FROM trace_mentions m WHERE m.trace_id = t.id
                        AND m.mentioned_user_id = $2 AND m.removed_at IS NULL)
                    OR EXISTS (SELECT 1 FROM posts p WHERE p.source_trace_id = t.id
                        AND trace_publication_readable(p.id, $2))
                )
              ))
        )
        SELECT
          (SELECT COUNT(*) FROM readable_traces)::bigint AS traces_count,
          (SELECT MIN(finalized_at) FROM readable_traces) AS oldest_trace_finalized_at,
          (SELECT COUNT(*) FROM albums a WHERE a.owner_user_id = $1 AND (
              a.owner_user_id = $2 OR (a.visibility = 'PUBLISHED' AND EXISTS (
                  SELECT 1 FROM album_items i JOIN posts p ON p.source_trace_id = i.trace_id
                  WHERE i.album_id = a.id AND trace_publication_readable(p.id, $2)
              ))
          ))::bigint AS albums_count,
          (SELECT COUNT(*) FROM documents WHERE documents.owner_user_id = $1
              AND documents.status = 'ACTIVE' AND {})::bigint AS documents_count",
        readable_document_predicate(viewer),
    );
    let row = diesel::sql_query(query)
        .bind::<SqlUuid, _>(user_id)
        .bind::<SqlUuid, _>(viewer)
        .get_result::<CountsRow>(&mut conn)?;
    Ok(Json(UserContentCounts {
        traces_count: row.traces_count,
        albums_count: row.albums_count,
        documents_count: row.documents_count,
        oldest_trace_finalized_at: row.oldest_trace_finalized_at.map(|date| date.and_utc()),
    }))
}
