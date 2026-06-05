use chrono::NaiveDateTime;
use diesel::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    album::AlbumCompletionStatus,
    document::DocumentStatus,
    error::PpdcError,
    post::{PostSourceRef, PostStatus},
    post_grant::PostGrant,
    trace::TraceStatus,
};
use crate::schema::{albums, documents, posts, traces};

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

#[derive(Debug, Clone)]
struct SourceProjection {
    source_kind: FeedSourceKind,
    source_id: Uuid,
    journal_id: Option<Uuid>,
    title: String,
    subtitle: String,
    content: String,
    cover_image_asset_id: Option<Uuid>,
}

fn load_source_projection_map(
    rows: &[FeedPostRow],
    conn: &mut PgConnection,
) -> Result<HashMap<PostSourceRef, SourceProjection>, diesel::result::Error> {
    let mut trace_ids = Vec::new();
    let mut document_ids = Vec::new();
    let mut album_ids = Vec::new();

    for row in rows {
        if let Some(trace_id) = row.2 {
            trace_ids.push(trace_id);
        }
        if let Some(document_id) = row.3 {
            document_ids.push(document_id);
        }
        if let Some(album_id) = row.4 {
            album_ids.push(album_id);
        }
    }

    let mut projections = HashMap::new();

    if !trace_ids.is_empty() {
        let rows = traces::table
            .filter(traces::id.eq_any(trace_ids))
            .select((
                traces::id,
                traces::journal_id,
                traces::title,
                traces::subtitle,
                traces::content,
                traces::content_image_asset_id,
                traces::status,
            ))
            .load::<(Uuid, Uuid, String, String, String, Option<Uuid>, String)>(conn)?;

        projections.extend(rows.into_iter().filter_map(
            |(trace_id, journal_id, title, subtitle, content, cover_image_asset_id, status_raw)| {
                if TraceStatus::from_db(&status_raw) != TraceStatus::Finalized {
                    return None;
                }
                Some((
                    PostSourceRef::Trace(trace_id),
                    SourceProjection {
                        source_kind: FeedSourceKind::Trace,
                        source_id: trace_id,
                        journal_id: Some(journal_id),
                        title,
                        subtitle,
                        content,
                        cover_image_asset_id,
                    },
                ))
            },
        ));
    }

    if !document_ids.is_empty() {
        let rows = documents::table
            .filter(documents::id.eq_any(document_ids))
            .select((
                documents::id,
                documents::title,
                documents::subtitle,
                documents::description,
                documents::content,
                documents::cover_image_asset_id,
                documents::status,
            ))
            .load::<(
                Uuid,
                String,
                String,
                String,
                Option<String>,
                Option<Uuid>,
                String,
            )>(conn)?;

        projections.extend(rows.into_iter().filter_map(
            |(
                document_id,
                title,
                subtitle,
                description,
                content,
                cover_image_asset_id,
                status_raw,
            )| {
                if DocumentStatus::from_db(&status_raw) != DocumentStatus::Active {
                    return None;
                }
                Some((
                    PostSourceRef::Document(document_id),
                    SourceProjection {
                        source_kind: FeedSourceKind::Document,
                        source_id: document_id,
                        journal_id: None,
                        title,
                        subtitle,
                        content: content.unwrap_or(description),
                        cover_image_asset_id,
                    },
                ))
            },
        ));
    }

    if !album_ids.is_empty() {
        let rows = albums::table
            .filter(albums::id.eq_any(album_ids))
            .select((
                albums::id,
                albums::title,
                albums::subtitle,
                albums::content,
                albums::cover_image_asset_id,
                albums::completion_status,
            ))
            .load::<(Uuid, String, String, String, Option<Uuid>, String)>(conn)?;

        projections.extend(rows.into_iter().filter_map(
            |(album_id, title, subtitle, content, cover_image_asset_id, completion_status_raw)| {
                if AlbumCompletionStatus::from_db(&completion_status_raw)
                    == AlbumCompletionStatus::Archived
                {
                    return None;
                }
                Some((
                    PostSourceRef::Album(album_id),
                    SourceProjection {
                        source_kind: FeedSourceKind::Album,
                        source_id: album_id,
                        journal_id: None,
                        title,
                        subtitle,
                        content,
                        cover_image_asset_id,
                    },
                ))
            },
        ));
    }

    Ok(projections)
}

pub fn find_feed_items_paginated(
    viewer_user_id: Uuid,
    offset: i64,
    limit: i64,
    pool: &DbPool,
) -> Result<(Vec<FeedItem>, i64), PpdcError> {
    let visible_post_ids = PostGrant::find_visible_post_ids_for_user(viewer_user_id, pool)?;
    if visible_post_ids.is_empty() {
        return Ok((vec![], 0));
    }

    let mut conn = pool.get()?;
    let rows = posts::table
        .filter(posts::status.eq(PostStatus::Published.to_db()))
        .filter(posts::user_id.ne(viewer_user_id))
        .filter(posts::id.eq_any(visible_post_ids))
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
        .load::<FeedPostRow>(&mut conn)?;

    let projections = load_source_projection_map(&rows, &mut conn)?;
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
                Some(FeedItem {
                    post_id,
                    source_kind: projection.source_kind,
                    source_id: projection.source_id,
                    owner_user_id,
                    journal_id: projection.journal_id,
                    status: PostStatus::from_db(&status_raw),
                    publishing_date,
                    title: projection.title.clone(),
                    subtitle: projection.subtitle.clone(),
                    content: projection.content.clone(),
                    cover_image_asset_id: projection.cover_image_asset_id,
                    created_at,
                    updated_at,
                })
            },
        )
        .collect::<Vec<_>>();

    let total = items.len() as i64;
    let items = items
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect::<Vec<_>>();

    Ok((items, total))
}
