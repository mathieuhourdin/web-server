use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types::{Integer, Uuid as SqlUuid};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    db::DbPool,
    entities_v2::{
        asset::Asset,
        error::{ErrorType, PpdcError},
    },
    schema::trace_source_assets,
};

#[derive(Serialize, Debug, Clone)]
pub struct TraceSourceAsset {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub asset_id: Uuid,
    pub position: i32,
    pub created_at: NaiveDateTime,
}

#[derive(Serialize, Debug, Clone)]
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

    fn readable_view(source_asset: Self, asset: Asset) -> TraceSourceAssetReadableView {
        TraceSourceAssetReadableView {
            id: source_asset.id,
            trace_id: source_asset.trace_id,
            asset_id: source_asset.asset_id,
            position: source_asset.position,
            created_at: source_asset.created_at,
            image_width: asset.image_width,
            image_height: asset.image_height,
            original_filename: asset.original_filename,
            mime_type: asset.mime_type,
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
        let source_assets = Self::find_for_trace(trace_id, pool)?;
        let asset_ids = source_assets
            .iter()
            .map(|source_asset| source_asset.asset_id)
            .collect::<Vec<_>>();
        let assets_by_id = Asset::find_by_ids(&asset_ids, pool)?;

        source_assets
            .into_iter()
            .map(|source_asset| {
                let asset = assets_by_id
                    .get(&source_asset.asset_id)
                    .cloned()
                    .ok_or_else(|| {
                        PpdcError::new(
                            500,
                            ErrorType::InternalError,
                            "Trace source asset is missing its uploaded asset".to_string(),
                        )
                    })?;
                Ok(Self::readable_view(source_asset, asset))
            })
            .collect()
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
