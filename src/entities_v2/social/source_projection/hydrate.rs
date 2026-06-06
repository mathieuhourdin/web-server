use diesel::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::entities_v2::post::{Post, PostSourceRef};
use crate::entities_v2::{
    album::AlbumCompletionStatus, document::DocumentStatus, error::PpdcError, post::PostType,
    trace::TraceStatus,
};
use crate::schema::{albums, documents, traces};

use super::model::{SourceProjection, SourceProjectionKind, SourceProjectionState};

pub type SourceProjectionMap = HashMap<PostSourceRef, SourceProjection>;

pub fn collect_post_source_refs(posts: &[Post]) -> Vec<PostSourceRef> {
    let mut refs = posts
        .iter()
        .filter_map(Post::source_ref)
        .collect::<Vec<_>>();
    refs.sort_unstable_by_key(|source_ref| match source_ref {
        PostSourceRef::Trace(id) => (0_u8, *id),
        PostSourceRef::Document(id) => (1_u8, *id),
        PostSourceRef::Album(id) => (2_u8, *id),
    });
    refs.dedup();
    refs
}

pub fn load_source_projection_map(
    source_refs: &[PostSourceRef],
    conn: &mut PgConnection,
) -> Result<SourceProjectionMap, PpdcError> {
    let mut trace_ids = Vec::new();
    let mut document_ids = Vec::new();
    let mut album_ids = Vec::new();

    for source_ref in source_refs {
        match source_ref {
            PostSourceRef::Trace(id) => trace_ids.push(*id),
            PostSourceRef::Document(id) => document_ids.push(*id),
            PostSourceRef::Album(id) => album_ids.push(*id),
        }
    }

    let mut projections = HashMap::new();

    if !trace_ids.is_empty() {
        let rows = traces::table
            .filter(traces::id.eq_any(trace_ids))
            .select((
                traces::id,
                traces::journal_id.nullable(),
                traces::title,
                traces::subtitle,
                traces::content,
                traces::content_image_asset_id,
                traces::status,
            ))
            .load::<(
                Uuid,
                Option<Uuid>,
                String,
                String,
                String,
                Option<Uuid>,
                String,
            )>(conn)?;

        projections.extend(rows.into_iter().map(
            |(id, journal_id, title, subtitle, content, cover_image_asset_id, status_raw)| {
                (
                    PostSourceRef::Trace(id),
                    SourceProjection {
                        source_kind: SourceProjectionKind::Trace,
                        source_id: id,
                        journal_id,
                        title,
                        subtitle,
                        content,
                        cover_image_asset_id,
                        default_post_type: PostType::Idea,
                        state: SourceProjectionState::Trace(TraceStatus::from_db(&status_raw)),
                    },
                )
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
                documents::document_type,
                documents::status,
            ))
            .load::<(
                Uuid,
                String,
                String,
                String,
                Option<String>,
                Option<Uuid>,
                Option<String>,
                String,
            )>(conn)?;

        projections.extend(rows.into_iter().map(
            |(
                id,
                title,
                subtitle,
                description,
                content,
                cover_image_asset_id,
                document_type_raw,
                status_raw,
            )| {
                (
                    PostSourceRef::Document(id),
                    SourceProjection {
                        source_kind: SourceProjectionKind::Document,
                        source_id: id,
                        journal_id: None,
                        title,
                        subtitle,
                        content: content.unwrap_or(description),
                        cover_image_asset_id,
                        default_post_type: document_type_raw
                            .as_deref()
                            .map(PostType::from_db)
                            .unwrap_or(PostType::Idea),
                        state: SourceProjectionState::Document(DocumentStatus::from_db(
                            &status_raw,
                        )),
                    },
                )
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

        projections.extend(rows.into_iter().map(
            |(id, title, subtitle, content, cover_image_asset_id, completion_status_raw)| {
                (
                    PostSourceRef::Album(id),
                    SourceProjection {
                        source_kind: SourceProjectionKind::Album,
                        source_id: id,
                        journal_id: None,
                        title,
                        subtitle,
                        content,
                        cover_image_asset_id,
                        default_post_type: PostType::ResourceList,
                        state: SourceProjectionState::Album(AlbumCompletionStatus::from_db(
                            &completion_status_raw,
                        )),
                    },
                )
            },
        ));
    }

    Ok(projections)
}

pub fn apply_source_projection_to_post(post: &mut Post, projections: &SourceProjectionMap) {
    let Some(source_ref) = post.source_ref() else {
        return;
    };
    let Some(projection) = projections.get(&source_ref) else {
        return;
    };

    if matches!(post.post_type, PostType::Idea) {
        post.post_type = projection.default_post_type;
    }
}
