//! A plume acknowledges writing, independently of its publications.
use super::model::{Trace, TraceStatus};
use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        session::Session,
        user::{User, UserPublicResponse},
    },
    pagination::{PaginatedResponse, PaginationParams},
};
use axum::{
    extract::{Extension, Path, Query},
    Json,
};
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Bool, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PlumeSummary {
    pub plume_count: i64,
    pub plumed_by_me: bool,
}

#[derive(QueryableByName)]
struct SummaryRow {
    #[diesel(sql_type = SqlUuid)]
    trace_id: Uuid,
    #[diesel(sql_type = BigInt)]
    plume_count: i64,
    #[diesel(sql_type = Bool)]
    plumed_by_me: bool,
}

pub(crate) trait PlumeTarget {
    fn plume_trace_id(&self) -> Option<Uuid>;
    fn set_plumes(&mut self, summary: PlumeSummary);
}

pub(crate) fn hydrate<'a, T: PlumeTarget + 'a>(
    viewer: Uuid,
    items: impl IntoIterator<Item = &'a mut T>,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut items = items.into_iter().collect::<Vec<_>>();
    let ids = items
        .iter()
        .filter_map(|i| i.plume_trace_id())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut summaries = HashMap::new();
    if !ids.is_empty() {
        let rows = diesel::sql_query(
            "SELECT trace_id, COUNT(*)::bigint AS plume_count,
            BOOL_OR(user_id=$2) AS plumed_by_me FROM trace_plumes WHERE trace_id=ANY($1)
            GROUP BY trace_id",
        )
        .bind::<Array<SqlUuid>, _>(ids)
        .bind::<SqlUuid, _>(viewer)
        .load::<SummaryRow>(&mut pool.get()?)?;
        for r in rows {
            summaries.insert(
                r.trace_id,
                PlumeSummary {
                    plume_count: r.plume_count,
                    plumed_by_me: r.plumed_by_me,
                },
            );
        }
    }
    for item in &mut items {
        let summary = item
            .plume_trace_id()
            .and_then(|id| summaries.get(&id))
            .cloned()
            .unwrap_or_default();
        item.set_plumes(summary);
    }
    Ok(())
}

macro_rules! target {
    ($ty:ty, $item:ident, $id:expr) => {
        impl PlumeTarget for $ty {
            fn plume_trace_id(&self) -> Option<Uuid> {
                let $item = self;
                $id
            }
            fn set_plumes(&mut self, summary: PlumeSummary) {
                self.plume = summary;
            }
        }
    };
}
target!(super::model::Trace, item, Some(item.id));
target!(
    super::model::TraceListItem,
    item,
    Some(item.source_trace_id.unwrap_or(item.id))
);
target!(
    super::model::TraceReadableView,
    item,
    Some(item.source_trace_id.unwrap_or(item.id))
);
target!(
    crate::entities_v2::feed::FeedItem,
    item,
    matches!(
        item.source_kind,
        crate::entities_v2::feed::FeedSourceKind::Trace
    )
    .then_some(item.source_id)
);
target!(
    crate::entities_v2::album::AlbumItemTraceView,
    item,
    Some(item.id)
);

pub async fn put(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<PlumeSummary>, PpdcError> {
    let viewer = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(id, &pool)?;
    if !trace.user_can_read(viewer, &pool)? {
        return Err(PpdcError::unauthorized());
    }
    if trace.user_id == viewer || trace.status != TraceStatus::Finalized {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Plumes require someone else's finalized trace".into(),
        ));
    }
    // The unique key makes concurrent/repeated PUTs idempotent, without a
    // long-lived source-row lock or nested pool connections.
    diesel::sql_query(
        "INSERT INTO trace_plumes(trace_id,user_id)
        SELECT id,$2 FROM traces WHERE id=$1 AND status='FINALIZED' AND NOT is_encrypted
        ON CONFLICT(trace_id,user_id) DO NOTHING",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<SqlUuid, _>(viewer)
    .execute(&mut pool.get()?)?;
    summary(id, viewer, &pool).map(Json)
}

pub async fn delete(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<PlumeSummary>, PpdcError> {
    let viewer = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    // Retraction remains possible after losing access; do not disclose totals.
    diesel::sql_query("DELETE FROM trace_plumes WHERE trace_id=$1 AND user_id=$2")
        .bind::<SqlUuid, _>(id)
        .bind::<SqlUuid, _>(viewer)
        .execute(&mut pool.get()?)?;
    let trace = Trace::find_full_trace(id, &pool)?;
    if !trace.user_can_read(viewer, &pool)? {
        return Ok(Json(PlumeSummary::default()));
    }
    summary(id, viewer, &pool).map(Json)
}

fn summary(id: Uuid, viewer: Uuid, pool: &DbPool) -> Result<PlumeSummary, PpdcError> {
    let row = diesel::sql_query(
        "SELECT $1::uuid AS trace_id,COUNT(*)::bigint AS plume_count,
        COALESCE(BOOL_OR(user_id=$2),FALSE) AS plumed_by_me FROM trace_plumes WHERE trace_id=$1",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<SqlUuid, _>(viewer)
    .get_result::<SummaryRow>(&mut pool.get()?)?;
    Ok(PlumeSummary {
        plume_count: row.plume_count,
        plumed_by_me: row.plumed_by_me,
    })
}

#[derive(QueryableByName)]
struct ReaderRow {
    #[diesel(sql_type = diesel::sql_types::Nullable<SqlUuid>)]
    user_id: Option<Uuid>,
    #[diesel(sql_type = BigInt)]
    total: i64,
}

pub async fn readers(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<UserPublicResponse>>, PpdcError> {
    let viewer = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != viewer {
        return Err(PpdcError::unauthorized());
    }
    let p = params.validate()?;
    let rows=diesel::sql_query("WITH readers AS (SELECT user_id,created_at FROM trace_plumes WHERE trace_id=$1)
        SELECT page.user_id,total.total FROM (SELECT COUNT(*)::bigint AS total FROM readers) total
        LEFT JOIN LATERAL (SELECT user_id,created_at FROM readers ORDER BY created_at DESC,user_id LIMIT $2 OFFSET $3) page ON TRUE
        ORDER BY page.created_at DESC,page.user_id")
        .bind::<SqlUuid,_>(id).bind::<BigInt,_>(p.limit).bind::<BigInt,_>(p.offset)
        .load::<ReaderRow>(&mut pool.get()?)?;
    let total = rows.first().map(|r| r.total).unwrap_or(0);
    let ids = rows
        .into_iter()
        .filter_map(|r| r.user_id)
        .collect::<Vec<_>>();
    let users = User::find_many(&ids, &pool)?
        .into_iter()
        .map(|u| (u.id, u))
        .collect::<HashMap<_, _>>();
    let items = ids
        .iter()
        .filter_map(|id| users.get(id))
        .map(UserPublicResponse::from)
        .collect();
    Ok(Json(PaginatedResponse::new(items, p, total)))
}
