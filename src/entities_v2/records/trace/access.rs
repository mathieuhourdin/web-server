//! Owner-facing audit of a reader's effective access, across all journals.
use super::model::Trace;
use crate::{
    db::DbPool,
    entities_v2::{error::PpdcError, session::Session, user::User},
    pagination::{PaginatedResponse, PaginationParams},
};
use axum::{
    extract::{Extension, Path, Query},
    Json,
};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Nullable, Uuid as SqlUuid},
};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type = Nullable<SqlUuid>)]
    id: Option<Uuid>,
    #[diesel(sql_type = BigInt)]
    total: i64,
}

pub async fn for_reader(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(reader): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Trace>>, PpdcError> {
    let owner = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    User::find(&reader, &pool)?;
    let p = params.validate()?;
    let rows=diesel::sql_query("WITH readable AS MATERIALIZED (
        SELECT t.id,t.finalized_at FROM traces t WHERE t.user_id=$1
        AND t.trace_type='USER_TRACE' AND t.status='FINALIZED'
        AND ($1=$2 OR (NOT t.is_encrypted AND automation_same_sandbox(t.user_id,$2)
          AND NOT EXISTS (SELECT 1 FROM user_blocks b WHERE
            (b.blocker_user_id=t.user_id AND b.blocked_user_id=$2)
            OR (b.blocked_user_id=t.user_id AND b.blocker_user_id=$2))
          AND (EXISTS (SELECT 1 FROM trace_mentions m WHERE m.trace_id=t.id
            AND m.mentioned_user_id=$2 AND m.removed_at IS NULL)
            OR EXISTS (SELECT 1 FROM posts post WHERE post.source_trace_id=t.id
                AND trace_publication_readable(post.id,$2)))))
        ) SELECT page.id,total.total FROM (SELECT COUNT(*)::bigint AS total FROM readable) total
        LEFT JOIN LATERAL (SELECT id,finalized_at FROM readable ORDER BY finalized_at DESC NULLS LAST,id DESC
            LIMIT $3 OFFSET $4) page ON TRUE ORDER BY page.finalized_at DESC NULLS LAST,page.id DESC")
        .bind::<SqlUuid,_>(owner).bind::<SqlUuid,_>(reader)
        .bind::<BigInt,_>(p.limit).bind::<BigInt,_>(p.offset)
        .load::<Row>(&mut pool.get()?)?;
    let total = rows.first().map(|r| r.total).unwrap_or(0);
    let ids = rows.into_iter().filter_map(|r| r.id).collect::<Vec<_>>();
    let mut by_id = Trace::find_full_traces_by_ids(&ids, &pool)?
        .into_iter()
        .map(|t| (t.id, t))
        .collect::<HashMap<_, _>>();
    let mut items = ids
        .iter()
        .filter_map(|id| by_id.remove(id))
        .collect::<Vec<_>>();
    // This is the owner's response: never hydrate another reader's private translation.
    super::continuation::hydrate_continuations(owner, items.iter_mut(), &pool)?;
    super::translation::hydrate_translations(owner, items.iter_mut(), &pool)?;
    super::plume::hydrate(owner, items.iter_mut(), &pool)?;
    Ok(Json(PaginatedResponse::new(items, p, total)))
}
