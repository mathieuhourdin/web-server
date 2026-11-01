//! A lightweight, viewer-scoped gallery of finalized journal trace images.
use super::model::Journal;
use crate::{
    db::DbPool,
    entities_v2::{error::PpdcError, session::Session, trace::model::TraceContentImage},
    pagination::{PaginatedResponse, PaginationParams, ValidatedPagination},
};
use axum::extract::{Extension, Json, Path, Query};
use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Integer, Nullable, Timestamp, Uuid as SqlUuid},
};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct JournalImage {
    pub trace_id: Uuid,
    pub finalized_at: Option<DateTime<Utc>>,
    pub image: TraceContentImage,
}

#[derive(QueryableByName)]
struct ImageRow {
    #[diesel(sql_type = SqlUuid)]
    trace_id: Uuid,
    #[diesel(sql_type = Nullable<Timestamp>)]
    finalized_at: Option<NaiveDateTime>,
    #[diesel(sql_type = SqlUuid)]
    asset_id: Uuid,
    #[diesel(sql_type = Nullable<Integer>)]
    width: Option<i32>,
    #[diesel(sql_type = Nullable<Integer>)]
    height: Option<i32>,
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

// EXISTS keeps one row per trace even when several social posts grant access.
// Public blog publications alone never grant private journal access.
const VISIBLE_IMAGES: &str = "
    FROM traces t
    JOIN journals j ON j.id=t.journal_id
    JOIN assets a ON a.id=t.content_image_asset_id
    WHERE t.journal_id=$1 AND t.trace_type='USER_TRACE' AND t.parent_trace_id IS NULL
      AND t.status='FINALIZED' AND a.status='READY' AND a.mime_type LIKE 'image/%'
      AND (j.user_id=$2 OR (
        j.status<>'ARCHIVED' AND NOT j.is_encrypted AND NOT t.is_encrypted
        AND automation_same_sandbox(t.user_id,$2)
        AND NOT EXISTS(SELECT 1 FROM user_blocks b WHERE
          (b.blocker_user_id=t.user_id AND b.blocked_user_id=$2)
          OR (b.blocked_user_id=t.user_id AND b.blocker_user_id=$2))
        AND (EXISTS(SELECT 1 FROM trace_mentions m WHERE m.trace_id=t.id
          AND m.mentioned_user_id=$2 AND m.removed_at IS NULL)
          OR EXISTS(SELECT 1 FROM posts p WHERE p.source_trace_id=t.id
            AND p.publication_channel_id IS NULL AND trace_publication_readable(p.id,$2)))
      ))";

impl Journal {
    pub fn images_paginated(
        &self,
        viewer: Uuid,
        pagination: ValidatedPagination,
        pool: &DbPool,
    ) -> Result<PaginatedResponse<JournalImage>, PpdcError> {
        // Authorize the journal independently: a readable journal can have no images.
        if !self.user_can_read(viewer, pool)? {
            return Err(PpdcError::unauthorized());
        }
        let mut conn = pool.get()?;
        let total = diesel::sql_query(format!("SELECT COUNT(*)::bigint AS count {VISIBLE_IMAGES}"))
            .bind::<SqlUuid, _>(self.id)
            .bind::<SqlUuid, _>(viewer)
            .get_result::<Count>(&mut conn)?
            .count;
        let rows = diesel::sql_query(format!(
            "SELECT t.id AS trace_id,t.finalized_at,a.id AS asset_id,
                a.image_width AS width,a.image_height AS height {VISIBLE_IMAGES}
             ORDER BY t.finalized_at DESC NULLS LAST,t.id DESC LIMIT $3 OFFSET $4"
        ))
        .bind::<SqlUuid, _>(self.id)
        .bind::<SqlUuid, _>(viewer)
        .bind::<BigInt, _>(pagination.limit)
        .bind::<BigInt, _>(pagination.offset)
        .load::<ImageRow>(&mut conn)?;
        let items = rows
            .into_iter()
            .map(|row| JournalImage {
                trace_id: row.trace_id,
                finalized_at: row.finalized_at.map(|d| d.and_utc()),
                image: TraceContentImage {
                    id: row.asset_id,
                    width: row.width,
                    height: row.height,
                },
            })
            .collect();
        Ok(PaginatedResponse::new(items, pagination, total))
    }
}

pub async fn list(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<JournalImage>>, PpdcError> {
    let viewer = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let journal = Journal::find_full(id, &pool)?;
    Ok(Json(journal.images_paginated(viewer, pagination, &pool)?))
}

#[cfg(test)]
mod tests;
