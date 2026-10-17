//! Personal translation messages on already-authorized trace response items.
//! The viewer is always the actual requester, never an inspected audience member.
use crate::{
    db::DbPool,
    entities_v2::{error::PpdcError, message::Message},
};
use std::collections::HashSet;
use uuid::Uuid;

pub(crate) trait TranslationTarget {
    fn translation_trace_id(&self) -> Option<Uuid>;
    fn set_translation_message(&mut self, message: Option<Message>);
}

pub(crate) fn hydrate_translations<'a, T: TranslationTarget + 'a>(
    viewer: Uuid,
    items: impl IntoIterator<Item = &'a mut T>,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut items = items.into_iter().collect::<Vec<_>>();
    let ids = items
        .iter()
        .filter_map(|item| item.translation_trace_id())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let by_id = Message::find_translation_replies_by_trace_ids(viewer, &ids, pool)?;
    for item in &mut items {
        let message = item
            .translation_trace_id()
            .and_then(|id| by_id.get(&id))
            .cloned();
        item.set_translation_message(message);
    }
    Ok(())
}

macro_rules! target {
    ($ty:ty,$item:ident,$id:expr) => {
        impl TranslationTarget for $ty {
            fn translation_trace_id(&self) -> Option<Uuid> {
                let $item = self;
                $id
            }
            fn set_translation_message(&mut self, message: Option<Message>) {
                self.translation_message = message;
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
