use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types::{Text, Uuid as SqlUuid};
use serde::Serialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    document::{Document, DocumentContentFormat, DocumentContentSource, PostType},
    error::{ErrorType, PpdcError},
    post::{Post, PostStatus},
    post_grant::PostGrant,
};
use crate::schema::trace_attachments;

#[derive(Serialize, Debug, Clone)]
pub struct TraceAttachment {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub document_id: Uuid,
    pub attachment_name: String,
    pub created_at: NaiveDateTime,
}

#[derive(Serialize, Debug, Clone)]
pub struct TraceAttachmentDocumentSummary {
    pub id: Uuid,
    pub document_type: Option<PostType>,
    pub content_source: DocumentContentSource,
    pub title: String,
    pub subtitle: String,
    pub description: String,
    pub author_name: Option<String>,
    pub content: Option<String>,
    pub content_format: Option<DocumentContentFormat>,
    pub asset_id: Option<Uuid>,
    pub external_content_url: Option<String>,
    pub cover_image_asset_id: Option<Uuid>,
    pub cover_image_external_url: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Debug, Clone)]
pub struct TraceAttachmentWithDocument {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub document_id: Uuid,
    pub attachment_name: String,
    pub created_at: NaiveDateTime,
    pub document: TraceAttachmentDocumentSummary,
}

pub type TraceAttachmentReadableView = TraceAttachmentWithDocument;

type TraceAttachmentTuple = (Uuid, Uuid, Uuid, String, NaiveDateTime);

#[derive(Debug, Clone)]
pub struct NewTraceAttachment {
    pub trace_id: Uuid,
    pub document_id: Uuid,
    pub attachment_name: String,
}

impl TraceAttachment {
    fn document_summary(document: Document) -> TraceAttachmentDocumentSummary {
        TraceAttachmentDocumentSummary {
            id: document.id,
            document_type: document.document_type,
            content_source: document.content_source,
            title: document.title,
            subtitle: document.subtitle,
            description: document.description,
            author_name: document.author_name,
            content: document.content,
            content_format: document.content_format,
            asset_id: document.asset_id,
            external_content_url: document.external_content_url,
            cover_image_asset_id: document.cover_image_asset_id,
            cover_image_external_url: document.cover_image_external_url,
            created_at: document.created_at,
            updated_at: document.updated_at,
        }
    }

    fn to_readable_view(
        attachment: TraceAttachment,
        document: Document,
    ) -> TraceAttachmentReadableView {
        TraceAttachmentReadableView {
            id: attachment.id,
            trace_id: attachment.trace_id,
            document_id: attachment.document_id,
            attachment_name: attachment.attachment_name,
            created_at: attachment.created_at,
            document: Self::document_summary(document),
        }
    }

    fn from_tuple(row: TraceAttachmentTuple) -> Self {
        let (id, trace_id, document_id, attachment_name, created_at) = row;
        Self {
            id,
            trace_id,
            document_id,
            attachment_name,
            created_at,
        }
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = pool.get()?;
        let row = trace_attachments::table
            .filter(trace_attachments::id.eq(id))
            .select((
                trace_attachments::id,
                trace_attachments::trace_id,
                trace_attachments::document_id,
                trace_attachments::attachment_name,
                trace_attachments::created_at,
            ))
            .first::<TraceAttachmentTuple>(&mut conn)
            .optional()?;

        row.map(Self::from_tuple).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Trace attachment not found".to_string(),
            )
        })
    }

    pub fn find_for_trace(trace_id: Uuid, pool: &DbPool) -> Result<Vec<Self>, PpdcError> {
        let mut conn = pool.get()?;
        let rows = trace_attachments::table
            .filter(trace_attachments::trace_id.eq(trace_id))
            .order((
                trace_attachments::created_at.asc(),
                trace_attachments::id.asc(),
            ))
            .select((
                trace_attachments::id,
                trace_attachments::trace_id,
                trace_attachments::document_id,
                trace_attachments::attachment_name,
                trace_attachments::created_at,
            ))
            .load::<TraceAttachmentTuple>(&mut conn)?;

        Ok(rows.into_iter().map(Self::from_tuple).collect())
    }

    pub fn find_with_documents_for_trace(
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<TraceAttachmentWithDocument>, PpdcError> {
        let attachments = Self::find_for_trace(trace_id, pool)?;
        attachments
            .into_iter()
            .map(|attachment| {
                let document = Document::find_full(attachment.document_id, pool)?;
                Ok(Self::to_readable_view(attachment, document))
            })
            .collect()
    }

    pub fn find_visible_for_post(
        post: &Post,
        viewer_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<TraceAttachmentReadableView>, PpdcError> {
        if post.user_id != viewer_user_id {
            if post.status != PostStatus::Published
                || !PostGrant::user_can_read_post(post, viewer_user_id, pool)?
            {
                return Err(PpdcError::unauthorized());
            }
        }

        let Some(trace_id) = post.source_trace_id else {
            return Ok(vec![]);
        };

        Self::find_readable_for_trace(trace_id, pool)
    }

    pub fn find_readable_for_trace(
        trace_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<TraceAttachmentReadableView>, PpdcError> {
        Self::find_with_documents_for_trace(trace_id, pool)
    }

    pub fn delete(id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
        let mut conn = pool.get()?;
        diesel::delete(trace_attachments::table.filter(trace_attachments::id.eq(id)))
            .execute(&mut conn)?;
        Ok(())
    }
}

impl NewTraceAttachment {
    pub fn create(self, pool: &DbPool) -> Result<TraceAttachment, PpdcError> {
        let mut conn = pool.get()?;
        let id = diesel::sql_query(
            "INSERT INTO trace_attachments (
                id,
                trace_id,
                document_id,
                attachment_name,
                created_at
             ) VALUES (
                uuid_generate_v4(),
                $1,
                $2,
                $3,
                NOW()
             )
             RETURNING id",
        )
        .bind::<SqlUuid, _>(self.trace_id)
        .bind::<SqlUuid, _>(self.document_id)
        .bind::<Text, _>(self.attachment_name)
        .get_result::<IdRow>(&mut conn)?;

        TraceAttachment::find(id.id, pool)
    }
}

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}
