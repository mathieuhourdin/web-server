use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::documents;

use super::model::{
    Document, DocumentContentFormat, DocumentContentSource, DocumentRole, DocumentStatus, PostType,
};

type DocumentTuple = (
    Uuid,
    Uuid,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    String,
    Option<String>,
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
            status,
            document_role,
            document_type,
            content_source,
            title,
            subtitle,
            description,
            author_name,
            content,
            content_format,
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
            status: DocumentStatus::from_db(&status),
            document_role: DocumentRole::from_db(&document_role),
            document_type: document_type.as_deref().map(PostType::from_db),
            content_source: DocumentContentSource::from_db(&content_source),
            title,
            subtitle,
            description,
            author_name,
            content,
            content_format: content_format
                .as_deref()
                .map(DocumentContentFormat::from_db),
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
    pub fn user_can_read(&self, viewer_user_id: Uuid) -> bool {
        self.owner_user_id == viewer_user_id
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<Document, PpdcError> {
        let mut conn = pool.get()?;
        let row = documents::table
            .filter(documents::id.eq(id))
            .select((
                documents::id,
                documents::owner_user_id,
                documents::status,
                documents::document_role,
                documents::document_type,
                documents::content_source,
                documents::title,
                documents::subtitle,
                documents::description,
                documents::author_name,
                documents::content,
                documents::content_format,
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
        Self::find_for_user_filtered_paginated(
            owner_user_id,
            None,
            None,
            None,
            Some(DocumentStatus::Active),
            offset,
            limit,
            pool,
        )
    }

    pub fn find_for_user_filtered_paginated(
        owner_user_id: Uuid,
        document_role: Option<DocumentRole>,
        document_type: Option<PostType>,
        content_source: Option<DocumentContentSource>,
        status: Option<DocumentStatus>,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Document>, i64), PpdcError> {
        let mut conn = pool.get()?;

        let mut count_query = documents::table
            .filter(documents::owner_user_id.eq(owner_user_id))
            .into_boxed();
        let mut query = documents::table
            .filter(documents::owner_user_id.eq(owner_user_id))
            .into_boxed();
        let status = status.unwrap_or(DocumentStatus::Active);

        if let Some(document_role) = document_role {
            count_query = count_query.filter(documents::document_role.eq(document_role.to_db()));
            query = query.filter(documents::document_role.eq(document_role.to_db()));
        }
        if let Some(document_type) = document_type {
            count_query = count_query.filter(documents::document_type.eq(document_type.to_db()));
            query = query.filter(documents::document_type.eq(document_type.to_db()));
        }
        if let Some(content_source) = content_source {
            count_query = count_query.filter(documents::content_source.eq(content_source.to_db()));
            query = query.filter(documents::content_source.eq(content_source.to_db()));
        }
        count_query = count_query.filter(documents::status.eq(status.to_db()));
        query = query.filter(documents::status.eq(status.to_db()));

        let total = count_query.count().get_result::<i64>(&mut conn)?;

        let rows = query
            .select((
                documents::id,
                documents::owner_user_id,
                documents::status,
                documents::document_role,
                documents::document_type,
                documents::content_source,
                documents::title,
                documents::subtitle,
                documents::description,
                documents::author_name,
                documents::content,
                documents::content_format,
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
