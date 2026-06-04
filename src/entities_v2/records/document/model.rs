use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::enums::{
    DocumentContentFormat, DocumentContentSource, DocumentRole, DocumentStatus,
};
pub use crate::entities_v2::post::PostType;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Document {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub status: DocumentStatus,
    pub document_role: DocumentRole,
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

#[derive(Deserialize, Debug, Clone)]
pub struct NewDocumentDto {
    #[serde(default)]
    pub status: Option<DocumentStatus>,
    pub document_role: DocumentRole,
    #[serde(default)]
    pub document_type: Option<PostType>,
    pub content_source: DocumentContentSource,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author_name: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub content_format: Option<DocumentContentFormat>,
    #[serde(default)]
    pub asset_id: Option<Uuid>,
    #[serde(default)]
    pub external_content_url: Option<String>,
    #[serde(default)]
    pub cover_image_asset_id: Option<Uuid>,
    #[serde(default)]
    pub cover_image_external_url: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct UpdateDocumentDto {
    pub status: Option<DocumentStatus>,
    pub document_role: Option<DocumentRole>,
    pub document_type: Option<Option<PostType>>,
    pub content_source: Option<DocumentContentSource>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub author_name: Option<Option<String>>,
    pub content: Option<Option<String>>,
    pub content_format: Option<Option<DocumentContentFormat>>,
    pub asset_id: Option<Option<Uuid>>,
    pub external_content_url: Option<Option<String>>,
    pub cover_image_asset_id: Option<Option<Uuid>>,
    pub cover_image_external_url: Option<Option<String>>,
}
