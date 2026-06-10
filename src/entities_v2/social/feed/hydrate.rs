use chrono::NaiveDateTime;
use diesel::dsl::{count_star, not};
use diesel::prelude::*;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    post::{PostSourceRef, PostStatus},
    post_grant::PostGrant,
    source_projection::{load_source_projection_map, SourceProjectionKind},
};
use crate::schema::{journals, posts, user_post_states, users};

use super::model::{FeedItem, FeedSourceKind};

type FeedPostRow = (
    Uuid,
    Uuid,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<NaiveDateTime>,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

pub fn find_feed_items_paginated(
    viewer_user_id: Uuid,
    seen: Option<bool>,
    offset: i64,
    limit: i64,
    pool: &DbPool,
) -> Result<(Vec<FeedItem>, i64), PpdcError> {
    let visible_post_ids = PostGrant::find_visible_post_ids_for_user(viewer_user_id, pool)?;
    if visible_post_ids.is_empty() {
        return Ok((vec![], 0));
    }

    let mut conn = pool.get()?;
    let seen_post_ids = if seen.is_some() {
        user_post_states::table
            .filter(user_post_states::user_id.eq(viewer_user_id))
            .select(user_post_states::post_id)
            .load::<Uuid>(&mut conn)?
    } else {
        vec![]
    };
    if seen == Some(true) && seen_post_ids.is_empty() {
        return Ok((vec![], 0));
    }

    // Feed eligibility is now fully captured by `posts.status = 'published'`: the
    // publication invariant (doc/publication.md) guarantees a published post always
    // has an eligible source, so no read-time source-state filter is needed. That
    // lets us filter, order, and paginate entirely in SQL and hydrate only the page.
    // The count query mirrors the page predicates so `total` stays exact. The
    // source-backed predicate keeps source-less custom posts out of the feed.
    let total: i64 = {
        let mut count_query = posts::table
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .filter(posts::user_id.ne(viewer_user_id))
            .filter(posts::id.eq_any(visible_post_ids.clone()))
            .filter(
                posts::source_trace_id
                    .is_not_null()
                    .or(posts::source_document_id.is_not_null())
                    .or(posts::source_album_id.is_not_null()),
            )
            .into_boxed();
        if let Some(seen) = seen {
            count_query = if seen {
                count_query.filter(posts::id.eq_any(seen_post_ids.clone()))
            } else {
                count_query.filter(not(posts::id.eq_any(seen_post_ids.clone())))
            };
        }
        count_query.select(count_star()).get_result(&mut conn)?
    };

    let mut page_query = posts::table
        .filter(posts::status.eq(PostStatus::Published.to_db()))
        .filter(posts::user_id.ne(viewer_user_id))
        .filter(posts::id.eq_any(visible_post_ids))
        .filter(
            posts::source_trace_id
                .is_not_null()
                .or(posts::source_document_id.is_not_null())
                .or(posts::source_album_id.is_not_null()),
        )
        .into_boxed();
    if let Some(seen) = seen {
        page_query = if seen {
            page_query.filter(posts::id.eq_any(seen_post_ids))
        } else {
            page_query.filter(not(posts::id.eq_any(seen_post_ids)))
        };
    }
    let rows = page_query
        .select((
            posts::id,
            posts::user_id,
            posts::source_trace_id,
            posts::source_document_id,
            posts::source_album_id,
            posts::publishing_date,
            posts::status,
            posts::created_at,
            posts::updated_at,
        ))
        .order(posts::publishing_date.desc().nulls_last())
        .then_order_by(posts::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load::<FeedPostRow>(&mut conn)?;

    let source_refs = rows
        .iter()
        .filter_map(|row| {
            if let Some(trace_id) = row.2 {
                Some(PostSourceRef::Trace(trace_id))
            } else if let Some(document_id) = row.3 {
                Some(PostSourceRef::Document(document_id))
            } else {
                row.4.map(PostSourceRef::Album)
            }
        })
        .collect::<Vec<_>>();
    let projections = load_source_projection_map(&source_refs, &mut conn)?;

    let owner_user_ids: Vec<Uuid> = rows
        .iter()
        .map(|row| row.1)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let display_name_map: HashMap<Uuid, String> = users::table
        .filter(users::id.eq_any(&owner_user_ids))
        .select((
            users::id,
            users::first_name,
            users::last_name,
            users::pseudonym,
            users::pseudonymized,
        ))
        .load::<(Uuid, String, String, String, bool)>(&mut conn)?
        .into_iter()
        .map(|(id, first_name, last_name, pseudonym, pseudonymized)| {
            let name = if pseudonymized {
                pseudonym
            } else {
                format!("{} {}", first_name, last_name)
            };
            (id, name)
        })
        .collect();

    let journal_ids: Vec<Uuid> = projections
        .values()
        .filter(|p| p.source_kind == SourceProjectionKind::Trace)
        .filter_map(|p| p.journal_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let journal_title_map: HashMap<Uuid, String> = if journal_ids.is_empty() {
        HashMap::new()
    } else {
        journals::table
            .filter(journals::id.eq_any(&journal_ids))
            .select((journals::id, journals::title))
            .load::<(Uuid, String)>(&mut conn)?
            .into_iter()
            .collect()
    };

    let items = rows
        .into_iter()
        .filter_map(
            |(
                post_id,
                owner_user_id,
                source_trace_id,
                source_document_id,
                source_album_id,
                publishing_date,
                status_raw,
                created_at,
                updated_at,
            )| {
                let source_ref = if let Some(trace_id) = source_trace_id {
                    Some(PostSourceRef::Trace(trace_id))
                } else if let Some(document_id) = source_document_id {
                    Some(PostSourceRef::Document(document_id))
                } else {
                    source_album_id.map(PostSourceRef::Album)
                }?;

                let projection = projections.get(&source_ref)?;
                // The publication invariant should guarantee this never fires; if it
                // does, a published post has drifted out of sync with its source.
                // Log for observability but keep the item so SQL pagination stays exact.
                if !projection.is_feed_eligible() {
                    tracing::warn!(
                        target: "feed",
                        "published_post_with_ineligible_source post_id={} source={:?}",
                        post_id,
                        source_ref
                    );
                }
                Some(FeedItem {
                    post_id,
                    source_kind: match projection.source_kind {
                        SourceProjectionKind::Trace => FeedSourceKind::Trace,
                        SourceProjectionKind::Document => FeedSourceKind::Document,
                        SourceProjectionKind::Album => FeedSourceKind::Album,
                    },
                    source_id: projection.source_id,
                    owner_user_id,
                    journal_id: projection.journal_id,
                    status: PostStatus::from_db(&status_raw),
                    publishing_date,
                    title: projection.title.clone(),
                    subtitle: projection.subtitle.clone(),
                    content: projection.content.clone(),
                    cover_image_asset_id: projection.cover_image_asset_id,
                    owner_display_name: display_name_map
                        .get(&owner_user_id)
                        .cloned()
                        .unwrap_or_default(),
                    journal_title: projection
                        .journal_id
                        .and_then(|jid| journal_title_map.get(&jid).cloned()),
                    created_at,
                    updated_at,
                })
            },
        )
        .collect::<Vec<_>>();

    Ok((items, total))
}
