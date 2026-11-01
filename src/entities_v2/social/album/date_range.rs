use chrono::NaiveDateTime;
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Bool, Nullable, Timestamp, Uuid as SqlUuid},
};
use std::collections::HashMap;
use uuid::Uuid;

use super::model::{Album, AlbumStats, AlbumTraceDateRange};
use crate::{db::DbPool, entities_v2::error::PpdcError};

#[derive(QueryableByName)]
struct RangeRow {
    #[diesel(sql_type = SqlUuid)]
    album_id: Uuid,
    #[diesel(sql_type = Bool)]
    chronological: bool,
    #[diesel(sql_type = Nullable<Timestamp>)]
    date_from: Option<NaiveDateTime>,
    #[diesel(sql_type = Nullable<Timestamp>)]
    date_to: Option<NaiveDateTime>,
    #[diesel(sql_type = BigInt)]
    traces_count: i64,
    #[diesel(sql_type = Array<SqlUuid>)]
    journal_ids: Vec<Uuid>,
}

impl RangeRow {
    fn date_range(&self) -> Option<AlbumTraceDateRange> {
        if !self.chronological {
            return None;
        }
        Some(AlbumTraceDateRange {
            from: self.date_from?.and_utc(),
            to: self.date_to?.and_utc(),
        })
    }
}

/// Date ranges and statistics share one grouped query for the whole album page.
fn visible_metrics(ids: &[Uuid], viewer: Uuid, pool: &DbPool) -> Result<Vec<RangeRow>, PpdcError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(diesel::sql_query(
            "SELECT a.id AS album_id,a.ordering_mode='CHRONOLOGICAL' AS chronological,
                MIN(t.finalized_at) AS date_from,MAX(t.finalized_at) AS date_to,
                COUNT(*)::bigint AS traces_count,
                COALESCE(ARRAY_AGG(DISTINCT t.journal_id ORDER BY t.journal_id)
                    FILTER(WHERE t.journal_id IS NOT NULL),ARRAY[]::uuid[]) AS journal_ids
             FROM albums a JOIN album_items i ON i.album_id = a.id JOIN traces t ON t.id = i.trace_id
             WHERE a.id = ANY($1)
               AND (a.owner_user_id = $2 OR (a.visibility = 'PUBLISHED' AND EXISTS (
                   SELECT 1 FROM posts p WHERE p.publication_channel_id IS NULL AND p.source_trace_id = t.id AND trace_publication_readable(p.id, $2)
               )))
             GROUP BY a.id",
        ).bind::<Array<SqlUuid>, _>(ids).bind::<SqlUuid, _>(viewer)
            .load::<RangeRow>(&mut pool.get()?)?)
}

impl Album {
    pub(crate) fn visible_date_ranges_for_ids(
        ids: &[Uuid],
        viewer: Uuid,
        pool: &DbPool,
    ) -> Result<HashMap<Uuid, AlbumTraceDateRange>, PpdcError> {
        Ok(visible_metrics(ids, viewer, pool)?
            .into_iter()
            .filter_map(|row| row.date_range().map(|range| (row.album_id, range)))
            .collect())
    }

    pub fn hydrate_visible_date_ranges(
        mut albums: Vec<Album>,
        viewer: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Album>, PpdcError> {
        let ids = albums.iter().map(|a| a.id).collect::<Vec<_>>();
        let mut metrics = visible_metrics(&ids, viewer, pool)?
            .into_iter()
            .map(|row| (row.album_id, row))
            .collect::<HashMap<_, _>>();
        for album in &mut albums {
            let row = metrics.remove(&album.id);
            album.visible_trace_date_range = row.as_ref().and_then(RangeRow::date_range);
            album.stats = Some(
                row.map(|row| AlbumStats {
                    traces_count: row.traces_count,
                    journal_ids: row.journal_ids,
                })
                .unwrap_or_default(),
            );
        }
        Ok(albums)
    }

    pub fn with_visible_date_range(self, viewer: Uuid, pool: &DbPool) -> Result<Album, PpdcError> {
        Ok(Self::hydrate_visible_date_ranges(vec![self], viewer, pool)?.remove(0))
    }
}
