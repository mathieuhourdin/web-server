use uuid::Uuid;

use crate::entities_v2::{
    album::{Album, AlbumCompletionStatus},
    document::{Document, DocumentStatus},
    post::PostType,
    trace::{Trace, TraceStatus},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceProjectionKind {
    Trace,
    Document,
    Album,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceProjectionState {
    Trace(TraceStatus),
    Document(DocumentStatus),
    Album(AlbumCompletionStatus),
}

#[derive(Debug, Clone)]
pub struct SourceProjection {
    pub source_kind: SourceProjectionKind,
    pub source_id: Uuid,
    pub journal_id: Option<Uuid>,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub cover_image_asset_id: Option<Uuid>,
    pub default_post_type: PostType,
    pub state: SourceProjectionState,
}

impl SourceProjection {
    pub fn from_trace(trace: &Trace) -> Self {
        Self {
            source_kind: SourceProjectionKind::Trace,
            source_id: trace.id,
            journal_id: trace.journal_id,
            title: trace.title.clone(),
            subtitle: trace.subtitle.clone(),
            content: trace.content.clone(),
            cover_image_asset_id: trace.content_image_asset_id,
            default_post_type: PostType::Idea,
            state: SourceProjectionState::Trace(trace.status),
        }
    }

    pub fn from_document(document: &Document) -> Self {
        Self {
            source_kind: SourceProjectionKind::Document,
            source_id: document.id,
            journal_id: None,
            title: document.title.clone(),
            subtitle: document.subtitle.clone(),
            content: document
                .content
                .clone()
                .unwrap_or_else(|| document.description.clone()),
            cover_image_asset_id: document.cover_image_asset_id,
            default_post_type: document.document_type.unwrap_or(PostType::Idea),
            state: SourceProjectionState::Document(document.status),
        }
    }

    pub fn from_album(album: &Album) -> Self {
        Self {
            source_kind: SourceProjectionKind::Album,
            source_id: album.id,
            journal_id: None,
            title: album.title.clone(),
            subtitle: album.subtitle.clone(),
            content: album.content.clone(),
            cover_image_asset_id: album.cover_image_asset_id,
            default_post_type: PostType::ResourceList,
            state: SourceProjectionState::Album(album.completion_status),
        }
    }
    pub fn is_feed_eligible(&self) -> bool {
        match self.state {
            SourceProjectionState::Trace(status) => status == TraceStatus::Finalized,
            SourceProjectionState::Document(status) => status == DocumentStatus::Active,
            SourceProjectionState::Album(status) => status != AlbumCompletionStatus::Archived,
        }
    }
}
