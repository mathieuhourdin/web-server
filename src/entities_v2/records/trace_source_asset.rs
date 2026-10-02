use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types::{Integer, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    db::DbPool,
    entities_v2::error::{ErrorType, PpdcError},
    schema::{assets, trace_source_assets},
};

#[derive(Serialize, Debug, Clone)]
pub struct TraceSourceAsset {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub asset_id: Uuid,
    pub position: i32,
    pub created_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TraceSourceAssetReadableView {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub asset_id: Uuid,
    pub position: i32,
    pub created_at: NaiveDateTime,
    pub image_width: Option<i32>,
    pub image_height: Option<i32>,
    pub original_filename: String,
    pub mime_type: String,
}

type TraceSourceAssetTuple = (Uuid, Uuid, Uuid, i32, NaiveDateTime);

impl TraceSourceAsset {
    fn from_tuple(row: TraceSourceAssetTuple) -> Self {
        let (id, trace_id, asset_id, position, created_at) = row;
        Self {
            id,
            trace_id,
            asset_id,
            position,
            created_at,
        }
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = pool.get()?;
        let row = trace_source_assets::table
            .filter(trace_source_assets::id.eq(id))
            .select((
                trace_source_assets::id,
                trace_source_assets::trace_id,
                trace_source_assets::asset_id,
                trace_source_assets::position,
                trace_source_assets::created_at,
            ))
            .first::<TraceSourceAssetTuple>(&mut conn)
            .optional()?;
        row.map(Self::from_tuple).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Trace source asset not found".to_string(),
            )
        })
    }

    pub fn find_for_trace(trace_id: Uuid, pool: &DbPool) -> Result<Vec<Self>, PpdcError> {
        let mut conn = pool.get()?;
        let rows = trace_source_assets::table
            .filter(trace_source_assets::trace_id.eq(trace_id))
            .order((
                trace_source_assets::position.asc(),
                trace_source_assets::id.asc(),
            ))
            .select((
                trace_source_assets::id,
                trace_source_assets::trace_id,
                trace_source_assets::asset_id,
                trace_source_assets::position,
                trace_source_assets::created_at,
            ))
            .load::<TraceSourceAssetTuple>(&mut conn)?;
        Ok(rows.into_iter().map(Self::from_tuple).collect())
    }

    pub fn find_readable_for_trace(
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<TraceSourceAssetReadableView>, PpdcError> {
        Ok(Self::find_readable_by_trace_ids(&[trace_id], pool)?
            .remove(&trace_id)
            .unwrap_or_default())
    }

    /// Metadata only; callers must authorize the traces before exposing these results.
    /// A single joined query hydrates all source photos in a trace page.
    pub fn find_readable_by_trace_ids(
        trace_ids: &[Uuid],
        pool: &DbPool,
    ) -> Result<HashMap<Uuid, Vec<TraceSourceAssetReadableView>>, PpdcError> {
        let mut grouped = HashMap::<Uuid, Vec<TraceSourceAssetReadableView>>::new();
        if trace_ids.is_empty() {
            return Ok(grouped);
        }
        let mut conn = pool.get()?;
        let rows = trace_source_assets::table
            .inner_join(assets::table.on(assets::id.eq(trace_source_assets::asset_id)))
            .filter(trace_source_assets::trace_id.eq_any(trace_ids))
            .order((
                trace_source_assets::position.asc(),
                trace_source_assets::id.asc(),
            ))
            .select((
                trace_source_assets::id,
                trace_source_assets::trace_id,
                trace_source_assets::asset_id,
                trace_source_assets::position,
                trace_source_assets::created_at,
                assets::image_width,
                assets::image_height,
                assets::original_filename,
                assets::mime_type,
            ))
            .load::<(
                Uuid,
                Uuid,
                Uuid,
                i32,
                NaiveDateTime,
                Option<i32>,
                Option<i32>,
                String,
                String,
            )>(&mut conn)?;
        for (
            id,
            trace_id,
            asset_id,
            position,
            created_at,
            image_width,
            image_height,
            original_filename,
            mime_type,
        ) in rows
        {
            grouped
                .entry(trace_id)
                .or_default()
                .push(TraceSourceAssetReadableView {
                    id,
                    trace_id,
                    asset_id,
                    position,
                    created_at,
                    image_width,
                    image_height,
                    original_filename,
                    mime_type,
                });
        }
        Ok(grouped)
    }

    pub fn create(trace_id: Uuid, asset_id: Uuid, pool: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = pool.get()?;
        let id = diesel::sql_query(
            "WITH locked_trace AS (
                SELECT id FROM traces WHERE id = $1 FOR UPDATE
             ), next_position AS (
                SELECT COALESCE(MAX(position) + 1, 0) AS position
                FROM trace_source_assets
                WHERE trace_id = $1
             )
             INSERT INTO trace_source_assets (id, trace_id, asset_id, position, created_at)
             SELECT uuid_generate_v4(), $1, $2, next_position.position, NOW()
             FROM locked_trace CROSS JOIN next_position
             RETURNING id",
        )
        .bind::<SqlUuid, _>(trace_id)
        .bind::<SqlUuid, _>(asset_id)
        .get_result::<IdRow>(&mut conn)?;
        Self::find(id.id, pool)
    }

    pub fn replace_order(
        trace_id: Uuid,
        source_asset_ids: Vec<Uuid>,
        pool: &DbPool,
    ) -> Result<Vec<TraceSourceAssetReadableView>, PpdcError> {
        let current = Self::find_for_trace(trace_id, pool)?;
        let current_ids = current
            .iter()
            .map(|source_asset| source_asset.id)
            .collect::<std::collections::HashSet<_>>();
        let requested_ids = source_asset_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        if source_asset_ids.len() != requested_ids.len() || current_ids != requested_ids {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "source_asset_ids must contain every trace source asset exactly once".to_string(),
            ));
        }

        let mut conn = pool.get()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            diesel::sql_query(
                "UPDATE trace_source_assets SET position = position + 1000000 WHERE trace_id = $1",
            )
            .bind::<SqlUuid, _>(trace_id)
            .execute(conn)?;
            for (position, id) in source_asset_ids.iter().enumerate() {
                diesel::sql_query("UPDATE trace_source_assets SET position = $1 WHERE id = $2")
                    .bind::<Integer, _>(position as i32)
                    .bind::<SqlUuid, _>(*id)
                    .execute(conn)?;
            }
            Ok(())
        })?;
        Self::find_readable_for_trace(trace_id, pool)
    }

    pub fn delete(id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
        let mut conn = pool.get()?;
        diesel::delete(trace_source_assets::table.filter(trace_source_assets::id.eq(id)))
            .execute(&mut conn)?;
        Ok(())
    }
}

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}
