//! Provenance only: copying selected text never transfers conversation access.
use crate::{db::DbPool, entities_v2::error::PpdcError};
use diesel::{
    prelude::*,
    sql_types::{Array, Nullable, Uuid as SqlUuid},
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(QueryableByName)]
struct Source {
    #[diesel(sql_type=SqlUuid)]
    trace_id: Uuid,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    message_id: Option<Uuid>,
}

pub(crate) fn lock_sources(ids: Option<&[Uuid]>, conn: &mut PgConnection) -> Result<(), PpdcError> {
    let Some(ids) = ids else {
        return Ok(());
    };
    if ids.len() > 100 || ids.iter().copied().collect::<HashSet<_>>().len() != ids.len() {
        return Err(super::service::invalid(
            "Provide at most 100 distinct source_message_ids",
        ));
    }
    #[derive(QueryableByName)]
    struct Id {
        #[diesel(sql_type=SqlUuid)]
        _id: Uuid,
    }
    diesel::sql_query("SELECT id AS _id FROM conversations WHERE id IN(SELECT conversation_id FROM messages WHERE id=ANY($1)) ORDER BY id FOR SHARE")
        .bind::<Array<SqlUuid>,_>(ids).load::<Id>(conn)?;
    Ok(())
}
pub(crate) fn replace(
    trace: Uuid,
    owner: Uuid,
    ids: Option<&[Uuid]>,
    conn: &mut PgConnection,
) -> Result<(), PpdcError> {
    let Some(ids) = ids else {
        return Ok(());
    };
    if ids.len() > 100 || ids.iter().copied().collect::<HashSet<_>>().len() != ids.len() {
        return Err(super::service::invalid(
            "Provide at most 100 distinct source_message_ids",
        ));
    }
    // Unchanged provenance survives leaving a group; only new sources require access.
    #[derive(QueryableByName)]
    struct Id {
        #[diesel(sql_type=SqlUuid)]
        id: Uuid,
    }
    let existing =
        diesel::sql_query("SELECT message_id AS id FROM trace_message_sources WHERE trace_id=$1")
            .bind::<SqlUuid, _>(trace)
            .load::<Id>(conn)?
            .into_iter()
            .map(|r| r.id)
            .collect::<HashSet<_>>();
    // Lock source conversations in a stable order before checking membership.
    diesel::sql_query("SELECT id FROM conversations WHERE id IN(SELECT conversation_id FROM messages WHERE id=ANY($1)) ORDER BY id FOR SHARE")
        .bind::<Array<SqlUuid>,_>(ids).load::<Id>(conn)?;
    // Validate all sources before deleting anything. Missing, departed and
    // pre-join messages use the same authorization predicate as message reads.
    for id in ids {
        if !existing.contains(id) {
            super::service::ensure_message_readable_on_connection(*id, owner, conn)?;
            let processed = diesel::sql_query(
                "SELECT id FROM messages WHERE id=$1 AND processing_state='PROCESSED'",
            )
            .bind::<SqlUuid, _>(id)
            .get_result::<Id>(conn)
            .optional()?;
            if processed.is_none() {
                return Err(super::service::invalid(
                    "Only processed messages can be trace sources",
                ));
            }
        }
    }
    diesel::sql_query("DELETE FROM trace_message_sources WHERE trace_id=$1")
        .bind::<SqlUuid, _>(trace)
        .execute(conn)?;
    diesel::sql_query(
        "INSERT INTO trace_message_sources(trace_id,message_id) SELECT $1,unnest($2::uuid[])",
    )
    .bind::<SqlUuid, _>(trace)
    .bind::<Array<SqlUuid>, _>(ids)
    .execute(conn)?;
    Ok(())
}

pub trait OwnerSourceView {
    fn trace_id(&self) -> Uuid;
    fn owner_id(&self) -> Option<Uuid>;
    fn set_sources(&mut self, ids: Option<Vec<Uuid>>);
}
impl OwnerSourceView for crate::entities_v2::trace::Trace {
    fn trace_id(&self) -> Uuid {
        self.id
    }
    fn owner_id(&self) -> Option<Uuid> {
        Some(self.user_id)
    }
    fn set_sources(&mut self, ids: Option<Vec<Uuid>>) {
        self.source_message_ids = ids;
    }
}
impl OwnerSourceView for crate::entities_v2::trace::TraceListItem {
    fn trace_id(&self) -> Uuid {
        self.id
    }
    fn owner_id(&self) -> Option<Uuid> {
        self.user_id
    }
    fn set_sources(&mut self, ids: Option<Vec<Uuid>>) {
        self.source_message_ids = ids;
    }
}
impl OwnerSourceView for crate::entities_v2::trace::model::TraceReadableView {
    fn trace_id(&self) -> Uuid {
        self.id
    }
    fn owner_id(&self) -> Option<Uuid> {
        self.user_id
    }
    fn set_sources(&mut self, ids: Option<Vec<Uuid>>) {
        self.source_message_ids = ids;
    }
}
impl OwnerSourceView for crate::entities_v2::feed::FeedItem {
    fn trace_id(&self) -> Uuid {
        self.source_id
    }
    fn owner_id(&self) -> Option<Uuid> {
        (self.source_kind == crate::entities_v2::feed::FeedSourceKind::Trace)
            .then_some(self.owner_user_id)
    }
    fn set_sources(&mut self, ids: Option<Vec<Uuid>>) {
        self.source_message_ids = ids;
    }
}
pub fn hydrate_owner<'a, T: OwnerSourceView + 'a>(
    owner: Uuid,
    items: impl IntoIterator<Item = &'a mut T>,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let items = items.into_iter().collect::<Vec<_>>();
    let ids = items
        .iter()
        .filter(|t| t.owner_id() == Some(owner))
        .map(|t| t.trace_id())
        .collect::<Vec<_>>();
    let map = for_owner(&ids, owner, pool)?;
    for item in items {
        item.set_sources(if item.owner_id() == Some(owner) {
            map.get(&item.trace_id()).cloned()
        } else {
            None
        });
    }
    Ok(())
}
pub(crate) fn for_owner(
    ids: &[Uuid],
    owner: Uuid,
    pool: &DbPool,
) -> Result<HashMap<Uuid, Vec<Uuid>>, PpdcError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = diesel::sql_query(
        "SELECT t.id AS trace_id,s.message_id FROM traces t
        LEFT JOIN trace_message_sources s ON s.trace_id=t.id LEFT JOIN messages m ON m.id=s.message_id
        WHERE t.id=ANY($1) AND t.user_id=$2 ORDER BY m.conversation_id,m.position",
    )
    .bind::<Array<SqlUuid>, _>(ids)
    .bind::<SqlUuid, _>(owner)
    .load::<Source>(&mut pool.get()?)?;
    let mut map: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for row in rows {
        let ids = map.entry(row.trace_id).or_default();
        if let Some(id) = row.message_id {
            ids.push(id);
        }
    }
    Ok(map)
}
