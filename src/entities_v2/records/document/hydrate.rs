use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::documents;

use super::model::{Document, DocumentContentSource, DocumentRole, PostType};

type DocumentTuple = (
    Uuid,
    Uuid,
    String,
    Option<String>,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<Uuid>,
    Option<String>,
    Option<Uuid>,
    Option<String>,
    NaiveDateTime,
    NaiveDateTime,
);

impl From<DocumentTuple> for Document {
    fn from(row: DocumentTuple) -> Self {
        let (
            id,
            owner_user_id,
            document_role,
            document_type,
            content_source,
            title,
            subtitle,
            description,
            author_name,
            content,
            asset_id,
            external_content_url,
            cover_image_asset_id,
            cover_image_external_url,
            created_at,
            updated_at,
        ) = row;

        Self {
            id,
            owner_user_id,
            document_role: DocumentRole::from_db(&document_role),
            document_type: document_type.as_deref().map(PostType::from_db),
            content_source: DocumentContentSource::from_db(&content_source),
            title,
            subtitle,
            description,
            author_name,
            content,
            asset_id,
            external_content_url,
            cover_image_asset_id,
            cover_image_external_url,
            created_at,
            updated_at,
        }
    }
}

impl Document {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Document, PpdcError> {
        let mut conn = pool.get()?;
        let row = documents::table
            .filter(documents::id.eq(id))
            .select((
                documents::id,
                documents::owner_user_id,
                documents::document_role,
                documents::document_type,
                documents::content_source,
                documents::title,
                documents::subtitle,
                documents::description,
                documents::author_name,
                documents::content,
                documents::asset_id,
                documents::external_content_url,
                documents::cover_image_asset_id,
                documents::cover_image_external_url,
                documents::created_at,
                documents::updated_at,
            ))
            .first::<DocumentTuple>(&mut conn)?;
        Ok(row.into())
    }

    pub fn find_full(id: Uuid, pool: &DbPool) -> Result<Document, PpdcError> {
        Self::find(id, pool)
    }

    pub fn find_for_user_paginated(
        owner_user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Document>, i64), PpdcError> {
        let mut conn = pool.get()?;

        let total = documents::table
            .filter(documents::owner_user_id.eq(owner_user_id))
            .count()
            .get_result::<i64>(&mut conn)?;

        let rows = documents::table
            .filter(documents::owner_user_id.eq(owner_user_id))
            .select((
                documents::id,
                documents::owner_user_id,
                documents::document_role,
                documents::document_type,
                documents::content_source,
                documents::title,
                documents::subtitle,
                documents::description,
                documents::author_name,
                documents::content,
                documents::asset_id,
                documents::external_content_url,
                documents::cover_image_asset_id,
                documents::cover_image_external_url,
                documents::created_at,
                documents::updated_at,
            ))
            .order((documents::updated_at.desc(), documents::created_at.desc()))
            .offset(offset)
            .limit(limit)
            .load::<DocumentTuple>(&mut conn)?;

        Ok((rows.into_iter().map(Document::from).collect(), total))
    }
}
