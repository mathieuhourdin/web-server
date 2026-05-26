use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    asset::Asset,
    error::{ErrorType, PpdcError},
    post::{Post, PostAudienceRole, PostStatus},
    post_grant::PostGrant,
    trace::Trace,
};
use crate::schema::{album_items, albums, posts};

use super::model::{
    Album, AlbumCompletionStatus, AlbumItem, AlbumItemTraceView, AlbumItemView, AlbumOrderingMode,
    AlbumVisibility, NewAlbum,
};

type AlbumTuple = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<Uuid>,
    chrono::NaiveDateTime,
    chrono::NaiveDateTime,
);

type AlbumItemTuple = (Uuid, Uuid, Uuid, i32, chrono::NaiveDateTime);

fn tuple_to_album(row: AlbumTuple) -> Album {
    Album {
        id: row.0,
        owner_user_id: row.1,
        title: row.2,
        subtitle: row.3,
        content: row.4,
        ordering_mode: AlbumOrderingMode::from_db(&row.5),
        completion_status: AlbumCompletionStatus::from_db(&row.6),
        visibility: AlbumVisibility::from_db(&row.7),
        cover_asset_id: row.8,
        created_at: row.9,
        updated_at: row.10,
    }
}

fn tuple_to_album_item(row: AlbumItemTuple) -> AlbumItem {
    AlbumItem {
        id: row.0,
        album_id: row.1,
        trace_id: row.2,
        ordering_index: row.3,
        created_at: row.4,
    }
}

fn select_album_columns() -> (
    albums::id,
    albums::owner_user_id,
    albums::title,
    albums::subtitle,
    albums::content,
    albums::ordering_mode,
    albums::completion_status,
    albums::visibility,
    albums::cover_asset_id,
    albums::created_at,
    albums::updated_at,
) {
    (
        albums::id,
        albums::owner_user_id,
        albums::title,
        albums::subtitle,
        albums::content,
        albums::ordering_mode,
        albums::completion_status,
        albums::visibility,
        albums::cover_asset_id,
        albums::created_at,
        albums::updated_at,
    )
}

fn select_album_item_columns() -> (
    album_items::id,
    album_items::album_id,
    album_items::trace_id,
    album_items::ordering_index,
    album_items::created_at,
) {
    (
        album_items::id,
        album_items::album_id,
        album_items::trace_id,
        album_items::ordering_index,
        album_items::created_at,
    )
}

fn validate_cover_asset_owner(
    owner_user_id: Uuid,
    cover_asset_id: Option<Uuid>,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    if let Some(asset_id) = cover_asset_id {
        let asset = Asset::find(asset_id, pool)?;
        if asset.owner_user_id != owner_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Album cover asset must belong to the album owner".to_string(),
            ));
        }
        let is_image = matches!(
            asset.mime_type.as_str(),
            "image/jpeg" | "image/png" | "image/webp" | "image/gif"
        );
        if !is_image {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Album cover asset must be an image".to_string(),
            ));
        }
    }
    Ok(())
}

fn published_or_created_at(post: &Post) -> chrono::NaiveDateTime {
    post.publishing_date.unwrap_or(post.created_at)
}

fn audience_priority(post: &Post) -> i32 {
    match post.audience_role {
        PostAudienceRole::Restricted => 1,
        PostAudienceRole::Default => 0,
    }
}

fn is_better_album_post_candidate(candidate: &Post, current: &Post) -> bool {
    let candidate_priority = audience_priority(candidate);
    let current_priority = audience_priority(current);
    if candidate_priority != current_priority {
        return candidate_priority > current_priority;
    }

    let candidate_published_at = published_or_created_at(candidate);
    let current_published_at = published_or_created_at(current);
    if candidate_published_at != current_published_at {
        return candidate_published_at > current_published_at;
    }

    candidate.created_at > current.created_at
}

fn preferred_post_for_trace(
    trace_id: Uuid,
    visible_post_ids: Option<&[Uuid]>,
    pool: &DbPool,
) -> Result<Option<Post>, PpdcError> {
    if visible_post_ids
        .map(|post_ids| post_ids.is_empty())
        .unwrap_or(false)
    {
        return Ok(None);
    }

    let mut conn = pool.get()?;
    let mut query = posts::table
        .filter(posts::source_trace_id.eq(Some(trace_id)))
        .filter(posts::status.eq(PostStatus::Published.to_db()))
        .into_boxed();
    if let Some(visible_post_ids) = visible_post_ids {
        query = query.filter(posts::id.eq_any(visible_post_ids.to_vec()));
    }

    let post_ids = query.select(posts::id).load::<Uuid>(&mut conn)?;

    let mut selected_post: Option<Post> = None;
    for post_id in post_ids {
        let post = Post::find_full(post_id, pool)?;
        let should_replace = selected_post
            .as_ref()
            .map(|current| is_better_album_post_candidate(&post, current))
            .unwrap_or(true);
        if should_replace {
            selected_post = Some(post);
        }
    }

    Ok(selected_post)
}

fn album_item_view_from_trace_and_post(
    item: AlbumItem,
    trace: Trace,
    selected_post: Option<Post>,
    viewer_is_owner: bool,
) -> AlbumItemView {
    let trace_view = if viewer_is_owner {
        AlbumItemTraceView {
            id: trace.id,
            journal_id: trace.journal_id,
            title: trace.title,
            subtitle: trace.subtitle,
            content: trace.content,
            image_asset_id: trace.image_asset_id,
            interaction_date: trace.interaction_date,
            published_post_id: selected_post.as_ref().map(|post| post.id),
            publishing_date: selected_post.as_ref().and_then(|post| post.publishing_date),
            post_audience_role: selected_post.as_ref().map(|post| post.audience_role),
        }
    } else {
        let post = selected_post.expect("non-owner album item views require a visible post");
        AlbumItemTraceView {
            id: trace.id,
            journal_id: trace.journal_id,
            title: post.title,
            subtitle: post.subtitle,
            content: post.content,
            image_asset_id: post.image_asset_id,
            interaction_date: trace.interaction_date,
            published_post_id: Some(post.id),
            publishing_date: post.publishing_date,
            post_audience_role: Some(post.audience_role),
        }
    };

    AlbumItemView {
        id: item.id,
        album_id: item.album_id,
        trace_id: item.trace_id,
        ordering_index: item.ordering_index,
        created_at: item.created_at,
        trace: trace_view,
    }
}

impl Album {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Album, PpdcError> {
        let mut conn = pool.get()?;
        let row = albums::table
            .filter(albums::id.eq(id))
            .select(select_album_columns())
            .first::<AlbumTuple>(&mut conn)
            .optional()?;

        row.map(tuple_to_album)
            .ok_or_else(|| PpdcError::new(404, ErrorType::ApiError, "Album not found".to_string()))
    }

    pub fn create(payload: NewAlbum, pool: &DbPool) -> Result<Album, PpdcError> {
        validate_cover_asset_owner(payload.owner_user_id, payload.cover_asset_id, pool)?;
        let mut conn = pool.get()?;
        let id = diesel::insert_into(albums::table)
            .values((
                albums::owner_user_id.eq(payload.owner_user_id),
                albums::title.eq(payload.title),
                albums::subtitle.eq(payload.subtitle),
                albums::content.eq(payload.content),
                albums::ordering_mode.eq(payload.ordering_mode.to_db()),
                albums::completion_status.eq(payload.completion_status.to_db()),
                albums::visibility.eq(payload.visibility.to_db()),
                albums::cover_asset_id.eq(payload.cover_asset_id),
            ))
            .returning(albums::id)
            .get_result::<Uuid>(&mut conn)?;
        Album::find(id, pool)
    }

    pub fn update(self, pool: &DbPool) -> Result<Album, PpdcError> {
        validate_cover_asset_owner(self.owner_user_id, self.cover_asset_id, pool)?;
        let mut conn = pool.get()?;
        diesel::update(albums::table.filter(albums::id.eq(self.id)))
            .set((
                albums::title.eq(self.title),
                albums::subtitle.eq(self.subtitle),
                albums::content.eq(self.content),
                albums::ordering_mode.eq(self.ordering_mode.to_db()),
                albums::completion_status.eq(self.completion_status.to_db()),
                albums::visibility.eq(self.visibility.to_db()),
                albums::cover_asset_id.eq(self.cover_asset_id),
            ))
            .execute(&mut conn)?;
        Album::find(self.id, pool)
    }

    pub fn user_can_read(&self, viewer_user_id: Uuid, pool: &DbPool) -> Result<bool, PpdcError> {
        if self.owner_user_id == viewer_user_id {
            return Ok(true);
        }
        if self.visibility != AlbumVisibility::Published {
            return Ok(false);
        }

        let visible_post_ids = PostGrant::find_visible_post_ids_for_user(viewer_user_id, pool)?;
        if visible_post_ids.is_empty() {
            return Ok(false);
        }

        let mut conn = pool.get()?;
        let count = album_items::table
            .inner_join(
                posts::table.on(posts::source_trace_id.eq(album_items::trace_id.nullable())),
            )
            .filter(album_items::album_id.eq(self.id))
            .filter(posts::id.eq_any(visible_post_ids))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .count()
            .get_result::<i64>(&mut conn)?;
        Ok(count > 0)
    }

    pub fn find_for_user_paginated(
        viewer_user_id: Uuid,
        owner_user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Album>, i64), PpdcError> {
        let mut conn = pool.get()?;
        if viewer_user_id == owner_user_id {
            let total = albums::table
                .filter(albums::owner_user_id.eq(owner_user_id))
                .count()
                .get_result::<i64>(&mut conn)?;
            let rows = albums::table
                .filter(albums::owner_user_id.eq(owner_user_id))
                .select(select_album_columns())
                .order(albums::updated_at.desc())
                .offset(offset)
                .limit(limit)
                .load::<AlbumTuple>(&mut conn)?;
            return Ok((rows.into_iter().map(tuple_to_album).collect(), total));
        }

        let visible_post_ids = PostGrant::find_visible_post_ids_for_user(viewer_user_id, pool)?;
        if visible_post_ids.is_empty() {
            return Ok((vec![], 0));
        }

        let album_ids = albums::table
            .inner_join(album_items::table.on(album_items::album_id.eq(albums::id)))
            .inner_join(
                posts::table.on(posts::source_trace_id.eq(album_items::trace_id.nullable())),
            )
            .filter(albums::owner_user_id.eq(owner_user_id))
            .filter(albums::visibility.eq(AlbumVisibility::Published.to_db()))
            .filter(posts::id.eq_any(visible_post_ids))
            .filter(posts::status.eq(PostStatus::Published.to_db()))
            .select(albums::id)
            .distinct()
            .load::<Uuid>(&mut conn)?;

        let total = album_ids.len() as i64;
        if album_ids.is_empty() {
            return Ok((vec![], 0));
        }

        let rows = albums::table
            .filter(albums::id.eq_any(album_ids))
            .select(select_album_columns())
            .order(albums::updated_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<AlbumTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_album).collect(), total))
    }

    pub fn find_items_for_viewer(
        &self,
        viewer_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<AlbumItemView>, PpdcError> {
        let mut conn = pool.get()?;
        let viewer_is_owner = self.owner_user_id == viewer_user_id;
        let visible_post_ids = if viewer_is_owner {
            vec![]
        } else {
            PostGrant::find_visible_post_ids_for_user(viewer_user_id, pool)?
        };

        let rows = if viewer_is_owner {
            album_items::table
                .filter(album_items::album_id.eq(self.id))
                .select(select_album_item_columns())
                .load::<AlbumItemTuple>(&mut conn)?
        } else {
            if visible_post_ids.is_empty() {
                return Ok(vec![]);
            }
            album_items::table
                .inner_join(
                    posts::table.on(posts::source_trace_id.eq(album_items::trace_id.nullable())),
                )
                .filter(album_items::album_id.eq(self.id))
                .filter(posts::id.eq_any(visible_post_ids.clone()))
                .filter(posts::status.eq(PostStatus::Published.to_db()))
                .select(select_album_item_columns())
                .distinct()
                .load::<AlbumItemTuple>(&mut conn)?
        };

        let mut items = Vec::new();
        for row in rows {
            let item = tuple_to_album_item(row);
            let trace = Trace::find_full_trace(item.trace_id, pool)?;
            let selected_post = preferred_post_for_trace(
                item.trace_id,
                if viewer_is_owner {
                    None
                } else {
                    Some(&visible_post_ids)
                },
                pool,
            )?;
            if viewer_is_owner || selected_post.is_some() {
                items.push(album_item_view_from_trace_and_post(
                    item,
                    trace,
                    selected_post,
                    viewer_is_owner,
                ));
            }
        }

        match self.ordering_mode {
            AlbumOrderingMode::Chronological => {
                items.sort_by_key(|item| (item.trace.interaction_date, item.created_at));
            }
            AlbumOrderingMode::Manual => {
                items.sort_by_key(|item| (item.ordering_index, item.created_at));
            }
            AlbumOrderingMode::AddedAt => {
                items.sort_by_key(|item| item.created_at);
            }
        }
        Ok(items)
    }
}

impl AlbumItem {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<AlbumItem, PpdcError> {
        let mut conn = pool.get()?;
        let row = album_items::table
            .filter(album_items::id.eq(id))
            .select(select_album_item_columns())
            .first::<AlbumItemTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_album_item).ok_or_else(|| {
            PpdcError::new(404, ErrorType::ApiError, "Album item not found".to_string())
        })
    }

    pub fn create_or_update(
        album: &Album,
        trace_id: Uuid,
        ordering_index: i32,
        pool: &DbPool,
    ) -> Result<AlbumItem, PpdcError> {
        let trace = Trace::find_full_trace(trace_id, pool)?;
        if trace.user_id != album.owner_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Album items must reference traces owned by the album owner".to_string(),
            ));
        }

        let mut conn = pool.get()?;
        diesel::insert_into(album_items::table)
            .values((
                album_items::album_id.eq(album.id),
                album_items::trace_id.eq(trace_id),
                album_items::ordering_index.eq(ordering_index),
            ))
            .on_conflict((album_items::album_id, album_items::trace_id))
            .do_update()
            .set(album_items::ordering_index.eq(ordering_index))
            .execute(&mut conn)?;

        let item_id = album_items::table
            .filter(album_items::album_id.eq(album.id))
            .filter(album_items::trace_id.eq(trace_id))
            .select(album_items::id)
            .first::<Uuid>(&mut conn)?;
        AlbumItem::find(item_id, pool)
    }

    pub fn delete(album_id: Uuid, item_id: Uuid, pool: &DbPool) -> Result<AlbumItem, PpdcError> {
        let item = AlbumItem::find(item_id, pool)?;
        if item.album_id != album_id {
            return Err(PpdcError::new(
                404,
                ErrorType::ApiError,
                "Album item not found for this album".to_string(),
            ));
        }

        let mut conn = pool.get()?;
        diesel::delete(album_items::table.filter(album_items::id.eq(item_id)))
            .execute(&mut conn)?;
        Ok(item)
    }
}
