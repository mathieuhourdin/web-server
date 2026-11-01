//! Small, viewer-scoped aggregates; callers authorize journals before hydration.
use super::model::Journal;
use crate::{db::DbPool, entities_v2::error::PpdcError};
use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Nullable, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct JournalStats {
    pub traces_count: i64,
    pub oldest_trace_finalized_at: Option<DateTime<Utc>>,
    pub newest_trace_finalized_at: Option<DateTime<Utc>>,
}

#[derive(QueryableByName)]
struct StatsRow {
    #[diesel(sql_type=SqlUuid)]
    journal_id: Uuid,
    #[diesel(sql_type=BigInt)]
    traces_count: i64,
    #[diesel(sql_type=Nullable<Timestamp>)]
    oldest_trace_finalized_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    newest_trace_finalized_at: Option<NaiveDateTime>,
}

impl Journal {
    /// One query for the whole page; EXISTS avoids counting publications twice.
    pub fn hydrate_stats(
        mut journals: Vec<Self>,
        viewer: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Self>, PpdcError> {
        if journals.is_empty() {
            return Ok(journals);
        }
        let ids = journals.iter().map(|j| j.id).collect::<Vec<_>>();
        let rows=diesel::sql_query("SELECT t.journal_id,COUNT(*)::bigint AS traces_count,
            MIN(t.finalized_at) AS oldest_trace_finalized_at,MAX(t.finalized_at) AS newest_trace_finalized_at
            FROM traces t JOIN journals j ON j.id=t.journal_id
            WHERE t.journal_id=ANY($1) AND t.trace_type='USER_TRACE' AND t.status='FINALIZED'
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
                ))
            GROUP BY t.journal_id")
            .bind::<Array<SqlUuid>,_>(&ids).bind::<SqlUuid,_>(viewer).load::<StatsRow>(&mut pool.get()?)?;
        let mut stats = rows
            .into_iter()
            .map(|r| {
                (
                    r.journal_id,
                    JournalStats {
                        traces_count: r.traces_count,
                        oldest_trace_finalized_at: r.oldest_trace_finalized_at.map(|d| d.and_utc()),
                        newest_trace_finalized_at: r.newest_trace_finalized_at.map(|d| d.and_utc()),
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        for journal in &mut journals {
            journal.stats = Some(stats.remove(&journal.id).unwrap_or_default());
        }
        Ok(journals)
    }
    pub fn with_stats(self, viewer: Uuid, pool: &DbPool) -> Result<Self, PpdcError> {
        Ok(Self::hydrate_stats(vec![self], viewer, pool)?.remove(0))
    }
}

#[cfg(test)]
mod tests;
