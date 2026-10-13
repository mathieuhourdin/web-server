use chrono::NaiveDateTime;
use diesel::{
    prelude::*,
    sql_types::{Array, Timestamp, Uuid as SqlUuid},
};
use std::collections::HashMap;
use uuid::Uuid;

use super::model::{Album, AlbumOrderingMode, AlbumTraceDateRange};
use crate::{db::DbPool, entities_v2::error::PpdcError};

#[derive(QueryableByName)]
struct RangeRow {
    #[diesel(sql_type = SqlUuid)]
    album_id: Uuid,
    #[diesel(sql_type = Timestamp)]
    date_from: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    date_to: NaiveDateTime,
}

impl Album {
    pub(crate) fn visible_date_ranges_for_ids(
        ids: &[Uuid],
        viewer: Uuid,
        pool: &DbPool,
    ) -> Result<HashMap<Uuid, AlbumTraceDateRange>, PpdcError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = diesel::sql_query(
            "SELECT a.id AS album_id, MIN(t.finalized_at) AS date_from, MAX(t.finalized_at) AS date_to
             FROM albums a JOIN album_items i ON i.album_id = a.id JOIN traces t ON t.id = i.trace_id
             WHERE a.id = ANY($1) AND a.ordering_mode = 'CHRONOLOGICAL' AND t.finalized_at IS NOT NULL
               AND (a.owner_user_id = $2 OR (a.visibility = 'PUBLISHED' AND EXISTS (
                   SELECT 1 FROM posts p WHERE p.source_trace_id = t.id AND trace_publication_readable(p.id, $2)
               )))
             GROUP BY a.id",
        ).bind::<Array<SqlUuid>, _>(ids).bind::<SqlUuid, _>(viewer)
            .load::<RangeRow>(&mut pool.get()?)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.album_id,
                    AlbumTraceDateRange {
                        from: row.date_from.and_utc(),
                        to: row.date_to.and_utc(),
                    },
                )
            })
            .collect())
    }

    pub fn hydrate_visible_date_ranges(
        mut albums: Vec<Album>,
        viewer: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Album>, PpdcError> {
        let ids = albums
            .iter()
            .filter(|a| a.ordering_mode == AlbumOrderingMode::Chronological)
            .map(|a| a.id)
            .collect::<Vec<_>>();
        let ranges = Self::visible_date_ranges_for_ids(&ids, viewer, pool)?;
        for album in &mut albums {
            album.visible_trace_date_range = ranges.get(&album.id).cloned();
        }
        Ok(albums)
    }

    pub fn with_visible_date_range(self, viewer: Uuid, pool: &DbPool) -> Result<Album, PpdcError> {
        Ok(Self::hydrate_visible_date_ranges(vec![self], viewer, pool)?.remove(0))
    }
}
