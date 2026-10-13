use chrono::NaiveDateTime;
use diesel::dsl::{count_star, not};
use diesel::prelude::*;
use diesel::sql_types::{Array, BigInt, Nullable, Text, Timestamp, Uuid as SqlUuid};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    asset::Asset,
    error::PpdcError,
    post::{PostSourceRef, PostStatus},
    post_grant::PostGrant,
    source_projection::{load_source_projection_map, SourceProjectionKind},
    trace::TraceContentImage,
    user::User,
    user_post_state::{PostSeenByPreview, UserPostState, SEEN_BY_PREVIEW_LIMIT},
};
use crate::schema::{journals, posts, traces, user_post_states};

use super::model::{FeedItem, FeedSourceKind};

#[derive(QueryableByName)]
struct FeedCandidate {
    #[diesel(sql_type = Nullable<SqlUuid>)]
    post_id: Option<Uuid>,
    #[diesel(sql_type = SqlUuid)]
    owner_user_id: Uuid,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    source_trace_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    source_document_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    source_album_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<Timestamp>)]
    publishing_date: Option<NaiveDateTime>,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    feed_date: NaiveDateTime,
}

impl FeedCandidate {
    fn source_ref(&self) -> Option<PostSourceRef> {
        self.source_trace_id
            .map(PostSourceRef::Trace)
            .or_else(|| self.source_document_id.map(PostSourceRef::Document))
            .or_else(|| self.source_album_id.map(PostSourceRef::Album))
    }
}

#[derive(QueryableByName)]
struct FeedCount {
    #[diesel(sql_type = BigInt)]
    total: i64,
}

// Merge only lightweight candidates before global sorting/pagination. Album
// cards use album visibility + a readable child publication, not album-post ACLs.
// Backing album posts are deliberately absent to avoid duplicate cards/counts.
const FEED_CANDIDATES: &str = "
WITH feed_candidates AS (
    SELECT p.id AS entry_id, p.id AS post_id, p.user_id AS owner_user_id,
        p.source_trace_id, p.source_document_id, NULL::uuid AS source_album_id,
        p.publishing_date, p.status, p.created_at, p.updated_at,
        COALESCE(p.publishing_date, p.created_at) AS feed_date
    FROM posts p LEFT JOIN traces t ON t.id = p.source_trace_id
    WHERE p.status = 'PUBLISHED' AND p.user_id <> $1
      AND p.id = ANY($2) AND p.source_album_id IS NULL
      AND (p.source_trace_id IS NOT NULL OR p.source_document_id IS NOT NULL)
      AND t.trace_type IS DISTINCT FROM 'TRACE_COMPLEMENT'
      AND ($3::boolean IS NULL OR $3 = EXISTS (
          SELECT 1 FROM user_post_states s WHERE s.post_id = p.id AND s.user_id = $1
      ))
    UNION ALL
    SELECT a.id AS entry_id, NULL::uuid AS post_id, a.owner_user_id,
        NULL::uuid AS source_trace_id, NULL::uuid AS source_document_id,
        a.id AS source_album_id, NULL::timestamp AS publishing_date,
        'PUBLISHED'::text AS status, a.created_at, a.updated_at,
        a.created_at AS feed_date
    FROM albums a
    WHERE $3::boolean IS NULL AND a.owner_user_id <> $1
      AND a.visibility = 'PUBLISHED' AND a.completion_status <> 'ARCHIVED'
      AND EXISTS (
          SELECT 1 FROM album_items i JOIN posts child ON child.source_trace_id = i.trace_id
          WHERE i.album_id = a.id AND trace_publication_readable(child.id, $1)
      )
)";

pub fn find_feed_items_paginated(
    viewer_user_id: Uuid,
    seen: Option<bool>,
    offset: i64,
    limit: i64,
    pool: &DbPool,
) -> Result<(Vec<FeedItem>, i64), PpdcError> {
    let visible_post_ids = PostGrant::find_shared_post_ids_for_user(viewer_user_id, pool)?;
    let mut conn = pool.get()?;
    let total = diesel::sql_query(format!(
        "{FEED_CANDIDATES} SELECT COUNT(*)::bigint AS total FROM feed_candidates"
    ))
    .bind::<SqlUuid, _>(viewer_user_id)
    .bind::<Array<SqlUuid>, _>(&visible_post_ids)
    .bind::<Nullable<diesel::sql_types::Bool>, _>(seen)
    .get_result::<FeedCount>(&mut conn)?
    .total;
    let rows = diesel::sql_query(format!(
        "{FEED_CANDIDATES} SELECT * FROM feed_candidates
         ORDER BY feed_date DESC, created_at DESC, entry_id DESC, (post_id IS NULL) DESC
         OFFSET $4 LIMIT $5"
    ))
    .bind::<SqlUuid, _>(viewer_user_id)
    .bind::<Array<SqlUuid>, _>(&visible_post_ids)
    .bind::<Nullable<diesel::sql_types::Bool>, _>(seen)
    .bind::<BigInt, _>(offset)
    .bind::<BigInt, _>(limit)
    .load::<FeedCandidate>(&mut conn)?;

    drop(conn);

    Ok((hydrate_feed_candidates(rows, viewer_user_id, pool)?, total))
}

const MY_FEED_CANDIDATES: &str = "
WITH feed_candidates AS (
    SELECT t.id AS entry_id, p.id AS post_id, t.user_id AS owner_user_id,
        t.id AS source_trace_id, NULL::uuid AS source_document_id, NULL::uuid AS source_album_id,
        p.publishing_date, COALESCE(p.status, 'DRAFT') AS status, t.created_at, t.updated_at,
        COALESCE(t.finalized_at, t.interaction_date, t.created_at) AS feed_date
    FROM traces t LEFT JOIN LATERAL (
        SELECT id, status, publishing_date FROM posts
        WHERE source_trace_id = t.id AND user_id = t.user_id
        ORDER BY (status = 'PUBLISHED') DESC, created_at DESC, id DESC LIMIT 1
    ) p ON TRUE
    WHERE t.user_id = $1 AND t.trace_type = 'USER_TRACE' AND t.status = 'FINALIZED'
    UNION ALL
    SELECT a.id AS entry_id, NULL::uuid AS post_id, a.owner_user_id,
        NULL::uuid AS source_trace_id, NULL::uuid AS source_document_id,
        a.id AS source_album_id, NULL::timestamp AS publishing_date,
        CASE WHEN a.visibility = 'PUBLISHED' THEN 'PUBLISHED' ELSE 'DRAFT' END AS status,
        a.created_at, a.updated_at, a.created_at AS feed_date
    FROM albums a WHERE a.owner_user_id = $1 AND a.completion_status <> 'ARCHIVED'
)";

pub fn find_my_feed_items_paginated(
    user_id: Uuid,
    offset: i64,
    limit: i64,
    pool: &DbPool,
) -> Result<(Vec<FeedItem>, i64), PpdcError> {
    let mut conn = pool.get()?;
    let total = diesel::sql_query(format!(
        "{MY_FEED_CANDIDATES} SELECT COUNT(*)::bigint AS total FROM feed_candidates"
    ))
    .bind::<SqlUuid, _>(user_id)
    .get_result::<FeedCount>(&mut conn)?
    .total;
    let rows = diesel::sql_query(format!(
        "{MY_FEED_CANDIDATES} SELECT * FROM feed_candidates
         ORDER BY feed_date DESC, created_at DESC, entry_id DESC, (source_album_id IS NOT NULL) DESC
         OFFSET $2 LIMIT $3"
    ))
    .bind::<SqlUuid, _>(user_id)
    .bind::<BigInt, _>(offset)
    .bind::<BigInt, _>(limit)
    .load::<FeedCandidate>(&mut conn)?;
    drop(conn);
    let trace_ids = rows
        .iter()
        .filter_map(|row| row.source_trace_id)
        .collect::<Vec<_>>();
    let previews = UserPostState::find_seen_by_preview_by_trace_ids(
        user_id,
        &trace_ids,
        SEEN_BY_PREVIEW_LIMIT,
        pool,
    )?;
    let mut items = hydrate_feed_candidates(rows, user_id, pool)?;
    for item in &mut items {
        if item.source_kind == FeedSourceKind::Trace {
            item.seen_by_preview = Some(
                previews
                    .get(&item.source_id)
                    .cloned()
                    .unwrap_or_else(PostSeenByPreview::default),
            );
        }
    }
    Ok((items, total))
}

fn hydrate_feed_candidates(
    rows: Vec<FeedCandidate>,
    viewer_user_id: Uuid,
    pool: &DbPool,
) -> Result<Vec<FeedItem>, PpdcError> {
    let mut conn = pool.get()?;

    let source_refs = rows
        .iter()
        .filter_map(FeedCandidate::source_ref)
        .collect::<Vec<_>>();
    let projections = load_source_projection_map(&source_refs, &mut conn)?;
    drop(conn);

    let image_ids = projections
        .values()
        .filter_map(|p| p.cover_image_asset_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let image_by_id = Asset::find_image_dimensions_by_ids(&image_ids, pool)?
        .into_iter()
        .map(|image| {
            (
                image.id,
                TraceContentImage {
                    id: image.id,
                    width: image.width,
                    height: image.height,
                },
            )
        })
        .collect::<HashMap<_, _>>();

    let album_ids = rows
        .iter()
        .filter_map(|row| row.source_album_id)
        .collect::<Vec<_>>();
    let album_ranges = crate::entities_v2::album::Album::visible_date_ranges_for_ids(
        &album_ids,
        viewer_user_id,
        pool,
    )?;

    let mut displayed_user_ids: Vec<Uuid> = rows
        .iter()
        .map(|row| row.owner_user_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    displayed_user_ids.extend(
        projections
            .values()
            .filter_map(|projection| projection.original_author_user_id),
    );
    displayed_user_ids.sort_unstable();
    displayed_user_ids.dedup();
    let displayed_users = User::find_many(&displayed_user_ids, pool)?;
    let display_name_map = displayed_users
        .iter()
        .map(|user| (user.id, user.display_name()))
        .collect::<HashMap<_, _>>();
    let profile_picture_map = displayed_users
        .iter()
        .map(|user| (user.id, user.profile_picture_display_url(pool)))
        .collect::<HashMap<_, _>>();

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
        let mut conn = pool.get()?;
        journals::table
            .filter(journals::id.eq_any(&journal_ids))
            .select((journals::id, journals::title))
            .load::<(Uuid, String)>(&mut conn)?
            .into_iter()
            .collect()
    };

    let items = rows
        .into_iter()
        .filter_map(|row| {
            let source_ref = row.source_ref()?;
            let owner_user_id = row.owner_user_id;

            let projection = projections.get(&source_ref)?;
            // The publication invariant should guarantee this never fires; if it
            // does, a published post has drifted out of sync with its source.
            // Log for observability but keep the item so SQL pagination stays exact.
            if row.post_id.is_some() && !projection.is_feed_eligible() {
                tracing::warn!(
                    target: "feed",
                    "published_post_with_ineligible_source post_id={:?} source={:?}",
                    row.post_id,
                    source_ref
                );
            }
            Some(FeedItem {
                post_id: row.post_id,
                source_kind: match projection.source_kind {
                    SourceProjectionKind::Trace => FeedSourceKind::Trace,
                    SourceProjectionKind::Document => FeedSourceKind::Document,
                    SourceProjectionKind::Album => FeedSourceKind::Album,
                },
                source_id: projection.source_id,
                original_source_id: projection.original_source_id,
                original_author_user_id: projection.original_author_user_id,
                original_author_display_name: projection
                    .original_author_user_id
                    .and_then(|id| display_name_map.get(&id).cloned()),
                original_author_profile_picture_display_url: projection
                    .original_author_user_id
                    .and_then(|id| profile_picture_map.get(&id).cloned().flatten()),
                owner_user_id,
                journal_id: projection.journal_id,
                status: PostStatus::from_db(&row.status),
                publishing_date: row.publishing_date,
                feed_date: row.feed_date,
                title: projection.title.clone(),
                subtitle: projection.subtitle.clone(),
                content: projection.content.clone(),
                cover_image_asset_id: projection.cover_image_asset_id,
                seen_by_preview: None,
                cover_image: projection
                    .cover_image_asset_id
                    .and_then(|id| image_by_id.get(&id).cloned()),
                owner_display_name: display_name_map
                    .get(&owner_user_id)
                    .cloned()
                    .unwrap_or_default(),
                journal_title: projection
                    .journal_id
                    .and_then(|jid| journal_title_map.get(&jid).cloned()),
                created_at: row.created_at,
                updated_at: row.updated_at,
                visible_trace_date_range: row
                    .source_album_id
                    .and_then(|id| album_ranges.get(&id).cloned()),
            })
        })
        .collect::<Vec<_>>();

    Ok(items)
}

pub fn count_recent_unread_feed_items(
    viewer_user_id: Uuid,
    published_since: NaiveDateTime,
    pool: &DbPool,
) -> Result<i64, PpdcError> {
    let visible_post_ids = PostGrant::find_shared_post_ids_for_user(viewer_user_id, pool)?;
    if visible_post_ids.is_empty() {
        return Ok(0);
    }

    let mut conn = pool.get()?;
    let seen_post_ids = user_post_states::table
        .filter(user_post_states::user_id.eq(viewer_user_id))
        .select(user_post_states::post_id)
        .load::<Uuid>(&mut conn)?;

    let mut query = posts::table
        .left_join(traces::table.on(traces::id.nullable().eq(posts::source_trace_id)))
        .filter(posts::status.eq(PostStatus::Published.to_db()))
        .filter(posts::user_id.ne(viewer_user_id))
        .filter(diesel::dsl::sql::<diesel::sql_types::Bool>(
            "traces.trace_type IS DISTINCT FROM 'TRACE_COMPLEMENT'",
        ))
        .filter(posts::id.eq_any(visible_post_ids))
        .filter(posts::source_album_id.is_null())
        .filter(
            posts::source_trace_id
                .is_not_null()
                .or(posts::source_document_id.is_not_null()),
        )
        .filter(
            diesel::dsl::sql::<Timestamp>("COALESCE(posts.publishing_date, posts.created_at)")
                .ge(published_since),
        )
        .into_boxed();

    if !seen_post_ids.is_empty() {
        query = query.filter(not(posts::id.eq_any(seen_post_ids)));
    }

    Ok(query.select(count_star()).get_result(&mut conn)?)
}
