//! Hydrate authorized predecessor previews for already-authorized response items.
//! Never call per-trace access helpers here: one SQL query covers the whole page.
use crate::{db::DbPool, entities_v2::error::PpdcError};
use chrono::NaiveDateTime;
use diesel::{
    prelude::*,
    sql_types::{Array, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, QueryableByName)]
pub struct TraceContinuationPreview {
    #[diesel(sql_type=Nullable<Text>)]
    pub language_code: Option<String>,
    #[diesel(sql_type=SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type=Text)]
    pub title: String,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub finalized_at: Option<NaiveDateTime>,
}

pub(crate) trait ContinuationTarget {
    fn continuation_source_id(&self) -> Option<Uuid>;
    fn set_continuation(&mut self, preview: Option<TraceContinuationPreview>);
}

#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type=SqlUuid)]
    trace_id: Uuid,
    #[diesel(embed)]
    preview: TraceContinuationPreview,
}

/// The caller must already have authorized every supplied item. Filtering is
/// only applied to its predecessor: the link never grants access to that trace.
/// Continuations target ordinary traces only. Keep the owner/mention/block
/// predicate aligned with Trace::user_can_read; publication ACLs use its same
/// canonical trace_publication_readable function, including secondary publishers.
pub(crate) fn hydrate_continuations<'a, T: ContinuationTarget + 'a>(
    viewer: Uuid,
    items: impl IntoIterator<Item = &'a mut T>,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut items = items.into_iter().collect::<Vec<_>>();
    let ids = items
        .iter()
        .filter_map(|item| item.continuation_source_id())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if ids.is_empty() {
        for item in &mut items {
            item.set_continuation(None);
        }
        return Ok(());
    }
    let rows=diesel::sql_query(
        "WITH candidates AS MATERIALIZED (
            SELECT id AS trace_id,continues_trace_id FROM traces WHERE id=ANY($1)
        ), readable AS MATERIALIZED (
            SELECT parent.id,parent.title,parent.finalized_at,parent.language_code FROM traces parent
            JOIN (SELECT DISTINCT continues_trace_id FROM candidates WHERE continues_trace_id IS NOT NULL) refs
                ON refs.continues_trace_id=parent.id
            WHERE parent.trace_type='USER_TRACE' AND (
                parent.user_id=$2 OR (
                    parent.status='FINALIZED' AND NOT parent.is_encrypted
                    AND automation_same_sandbox(parent.user_id,$2)
                    AND NOT EXISTS(SELECT 1 FROM user_blocks b WHERE
                        (b.blocker_user_id=parent.user_id AND b.blocked_user_id=$2)
                        OR (b.blocked_user_id=parent.user_id AND b.blocker_user_id=$2))
                    AND (EXISTS(SELECT 1 FROM trace_mentions m WHERE m.trace_id=parent.id
                        AND m.mentioned_user_id=$2 AND m.removed_at IS NULL)
                        OR EXISTS(SELECT 1 FROM posts p WHERE p.source_trace_id=parent.id
                            AND trace_publication_readable(p.id,$2)))
                )
            )
        ) SELECT c.trace_id,r.id,r.title,r.finalized_at,r.language_code FROM candidates c
            JOIN readable r ON r.id=c.continues_trace_id"
    ).bind::<Array<SqlUuid>,_>(ids).bind::<SqlUuid,_>(viewer)
        .load::<Row>(&mut pool.get()?)?;
    let by_id = rows
        .into_iter()
        .map(|r| (r.trace_id, r.preview))
        .collect::<HashMap<_, _>>();
    for item in &mut items {
        let preview = item
            .continuation_source_id()
            .and_then(|id| by_id.get(&id))
            .cloned();
        item.set_continuation(preview);
    }
    Ok(())
}

macro_rules! target {
    ($ty:ty, $item:ident, $id:expr) => {
        impl ContinuationTarget for $ty {
            fn continuation_source_id(&self) -> Option<Uuid> {
                let $item = self;
                $id
            }
            fn set_continuation(&mut self, preview: Option<TraceContinuationPreview>) {
                self.continues_trace_id = preview.as_ref().map(|p| p.id);
                self.continues_from = preview;
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
