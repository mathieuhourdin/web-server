use diesel::prelude::*;
use diesel::sql_types::{Nullable, Text, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    asset::Asset,
    error::{ErrorType, PpdcError},
};

use super::model::{
    Document, DocumentContentFormat, DocumentContentSource, DocumentStatus, NewDocumentDto,
};

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn validate_document_storage(
    owner_user_id: Uuid,
    content_source: DocumentContentSource,
    content: &Option<String>,
    content_format: &Option<DocumentContentFormat>,
    asset_id: &Option<Uuid>,
    external_content_url: &Option<String>,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    match content_source {
        DocumentContentSource::DbContent => {
            if asset_id.is_some() || external_content_url.is_some() {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "db_content documents cannot define asset_id or external_content_url"
                        .to_string(),
                ));
            }
            if content.is_none() {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "db_content documents must define content".to_string(),
                ));
            }
            if content_format.is_none() {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "db_content documents must define content_format".to_string(),
                ));
            }
        }
        DocumentContentSource::InternalAsset => {
            let asset_id = asset_id.ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "internal_asset documents must define asset_id".to_string(),
                )
            })?;
            if content.is_some() || external_content_url.is_some() || content_format.is_some() {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "internal_asset documents cannot define content, content_format or external_content_url"
                        .to_string(),
                ));
            }
            let asset = Asset::find(asset_id, pool)?;
            if asset.owner_user_id != owner_user_id {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "asset_id must reference an asset owned by the document owner".to_string(),
                ));
            }
        }
        DocumentContentSource::ExternalUrl => {
            if content.is_some() || asset_id.is_some() || content_format.is_some() {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "external_url documents cannot define content, content_format or asset_id"
                        .to_string(),
                ));
            }
            if external_content_url.is_none() {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "external_url documents must define external_content_url".to_string(),
                ));
            }
        }
        DocumentContentSource::ReferenceOnly => {
            if content.is_some()
                || content_format.is_some()
                || asset_id.is_some()
                || external_content_url.is_some()
            {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "reference_only documents cannot define content, content_format, asset_id or external_content_url".to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn validate_document_cover(
    owner_user_id: Uuid,
    cover_image_asset_id: &Option<Uuid>,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    if let Some(asset_id) = cover_image_asset_id {
        let asset = Asset::find(*asset_id, pool)?;
        if asset.owner_user_id != owner_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "cover_image_asset_id must reference an asset owned by the document owner"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

impl Document {
    pub fn create(
        payload: NewDocumentDto,
        owner_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Document, PpdcError> {
        let title = payload.title.unwrap_or_default();
        let subtitle = payload.subtitle.unwrap_or_default();
        let description = payload.description.unwrap_or_default();
        let status = payload.status.unwrap_or(DocumentStatus::Active);
        let author_name = normalize_optional_text(payload.author_name);
        let content = match payload.content_source {
            DocumentContentSource::DbContent => Some(payload.content.unwrap_or_default()),
            _ => None,
        };
        let content_format = if payload.content_source == DocumentContentSource::DbContent {
            Some(
                payload
                    .content_format
                    .unwrap_or(DocumentContentFormat::PlainText),
            )
        } else {
            None
        };
        let asset_id = payload.asset_id;
        let external_content_url = normalize_optional_text(payload.external_content_url);
        let cover_image_asset_id = payload.cover_image_asset_id;
        let cover_image_external_url = normalize_optional_text(payload.cover_image_external_url);

        validate_document_storage(
            owner_user_id,
            payload.content_source,
            &content,
            &content_format,
            &asset_id,
            &external_content_url,
            pool,
        )?;
        validate_document_cover(owner_user_id, &cover_image_asset_id, pool)?;

        let mut conn = pool.get()?;
        let inserted = diesel::sql_query(
            "INSERT INTO documents (
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
                cover_image_external_url
            )
            VALUES (
                uuid_generate_v4(),
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15
            )
            RETURNING id",
        )
        .bind::<SqlUuid, _>(owner_user_id)
        .bind::<Text, _>(status.to_db())
        .bind::<Text, _>(payload.document_role.to_db())
        .bind::<Nullable<Text>, _>(payload.document_type.map(|kind| kind.to_db().to_string()))
        .bind::<Text, _>(payload.content_source.to_db())
        .bind::<Text, _>(title)
        .bind::<Text, _>(subtitle)
        .bind::<Text, _>(description)
        .bind::<Nullable<Text>, _>(author_name)
        .bind::<Nullable<Text>, _>(content)
        .bind::<Nullable<Text>, _>(content_format.map(|value| value.to_db().to_string()))
        .bind::<Nullable<SqlUuid>, _>(asset_id)
        .bind::<Nullable<Text>, _>(external_content_url)
        .bind::<Nullable<SqlUuid>, _>(cover_image_asset_id)
        .bind::<Nullable<Text>, _>(cover_image_external_url)
        .get_result::<IdRow>(&mut conn)?;

        Document::find_full(inserted.id, pool)
    }

    pub fn update(self, pool: &DbPool) -> Result<Document, PpdcError> {
        validate_document_storage(
            self.owner_user_id,
            self.content_source,
            &self.content,
            &self
                .content_format
                .or(if self.content_source == DocumentContentSource::DbContent {
                    Some(DocumentContentFormat::PlainText)
                } else {
                    None
                }),
            &self.asset_id,
            &self.external_content_url,
            pool,
        )?;
        validate_document_cover(self.owner_user_id, &self.cover_image_asset_id, pool)?;

        let content_format = if self.content_source == DocumentContentSource::DbContent {
            Some(
                self.content_format
                    .unwrap_or(DocumentContentFormat::PlainText),
            )
        } else {
            None
        };

        let mut conn = pool.get()?;
        let _ = diesel::sql_query(
            "UPDATE documents
             SET status = $2,
                 document_role = $3,
                 document_type = $4,
                 content_source = $5,
                 title = $6,
                 subtitle = $7,
                 description = $8,
                 author_name = $9,
                 content = $10,
                 content_format = $11,
                 asset_id = $12,
                 external_content_url = $13,
                 cover_image_asset_id = $14,
                 cover_image_external_url = $15,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind::<SqlUuid, _>(self.id)
        .bind::<Text, _>(self.status.to_db())
        .bind::<Text, _>(self.document_role.to_db())
        .bind::<Nullable<Text>, _>(self.document_type.map(|kind| kind.to_db().to_string()))
        .bind::<Text, _>(self.content_source.to_db())
        .bind::<Text, _>(self.title)
        .bind::<Text, _>(self.subtitle)
        .bind::<Text, _>(self.description)
        .bind::<Nullable<Text>, _>(normalize_optional_text(self.author_name))
        .bind::<Nullable<Text>, _>(self.content)
        .bind::<Nullable<Text>, _>(content_format.map(|value| value.to_db().to_string()))
        .bind::<Nullable<SqlUuid>, _>(self.asset_id)
        .bind::<Nullable<Text>, _>(normalize_optional_text(self.external_content_url))
        .bind::<Nullable<SqlUuid>, _>(self.cover_image_asset_id)
        .bind::<Nullable<Text>, _>(normalize_optional_text(self.cover_image_external_url))
        .execute(&mut conn)?;

        Document::find_full(self.id, pool)
    }
}
