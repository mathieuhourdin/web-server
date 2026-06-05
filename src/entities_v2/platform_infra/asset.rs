use std::time::Duration;

use axum::{
    debug_handler,
    extract::{Extension, Json, Multipart, Path},
};
use bytes::Bytes;
use chrono::{Duration as ChronoDuration, NaiveDateTime, Utc};
use diesel::prelude::*;
use google_cloud_auth::credentials::Builder as CredentialsBuilder;
use google_cloud_storage::{builder::storage::SignedUrlBuilder, client::Storage};
use http::Method;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    album::Album,
    document::Document,
    error::{ErrorType, PpdcError},
    session::Session,
    trace::Trace,
};
use crate::schema::{albums, assets, documents, trace_attachments, traces, users};

#[derive(Debug, Clone, Copy)]
enum AssetUsage {
    UserProfile,
    TraceImage { trace_id: Uuid },
    TraceAttachment { trace_id: Uuid },
    DocumentCover { document_id: Uuid },
    DocumentContentAsset { document_id: Uuid },
    AlbumCover { album_id: Uuid },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetStatus {
    Uploading,
    Ready,
    Failed,
}

impl AssetStatus {
    fn to_db(self) -> &'static str {
        match self {
            AssetStatus::Uploading => "UPLOADING",
            AssetStatus::Ready => "READY",
            AssetStatus::Failed => "FAILED",
        }
    }

    fn from_db(value: &str) -> Result<Self, PpdcError> {
        match value {
            "UPLOADING" | "uploading" => Ok(AssetStatus::Uploading),
            "READY" | "ready" => Ok(AssetStatus::Ready),
            "FAILED" | "failed" => Ok(AssetStatus::Failed),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid asset status: {}", value),
            )),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct Asset {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub bucket: String,
    pub object_key: String,
    pub mime_type: String,
    pub original_filename: String,
    pub size_bytes: i64,
    pub status: AssetStatus,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Debug, Clone)]
pub struct AssetUploadResponse {
    pub asset: Asset,
    pub signed_url: String,
    pub expires_at: NaiveDateTime,
}

#[derive(Serialize, Debug, Clone)]
pub struct AssetSignedUrlResponse {
    pub url: String,
    pub expires_at: NaiveDateTime,
}

type AssetTuple = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    String,
    i64,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

#[derive(Debug, Clone)]
struct NewAsset {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub bucket: String,
    pub object_key: String,
    pub mime_type: String,
    pub original_filename: String,
    pub size_bytes: i64,
    pub status: AssetStatus,
}

fn tuple_to_asset(row: AssetTuple) -> Result<Asset, PpdcError> {
    let (
        id,
        owner_user_id,
        bucket,
        object_key,
        mime_type,
        original_filename,
        size_bytes,
        status_raw,
        created_at,
        updated_at,
    ) = row;
    Ok(Asset {
        id,
        owner_user_id,
        bucket,
        object_key,
        mime_type,
        original_filename,
        size_bytes,
        status: AssetStatus::from_db(&status_raw)?,
        created_at,
        updated_at,
    })
}

fn select_asset_columns() -> (
    assets::id,
    assets::owner_user_id,
    assets::bucket,
    assets::object_key,
    assets::mime_type,
    assets::original_filename,
    assets::size_bytes,
    assets::status,
    assets::created_at,
    assets::updated_at,
) {
    (
        assets::id,
        assets::owner_user_id,
        assets::bucket,
        assets::object_key,
        assets::mime_type,
        assets::original_filename,
        assets::size_bytes,
        assets::status,
        assets::created_at,
        assets::updated_at,
    )
}

impl Asset {
    fn find_usages_with_conn(
        asset_id: Uuid,
        conn: &mut PgConnection,
    ) -> Result<Vec<AssetUsage>, PpdcError> {
        let mut usages = Vec::new();

        let profile_owner_ids = users::table
            .filter(users::profile_asset_id.eq(Some(asset_id)))
            .select(users::id)
            .load::<Uuid>(conn)?;
        if !profile_owner_ids.is_empty() {
            usages.push(AssetUsage::UserProfile);
        }

        let trace_image_ids = traces::table
            .filter(traces::image_asset_id.eq(Some(asset_id)))
            .select(traces::id)
            .load::<Uuid>(conn)?;
        usages.extend(
            trace_image_ids
                .into_iter()
                .map(|trace_id| AssetUsage::TraceImage { trace_id }),
        );

        let trace_attachment_ids = trace_attachments::table
            .filter(trace_attachments::asset_id.eq(asset_id))
            .select(trace_attachments::trace_id)
            .load::<Uuid>(conn)?;
        usages.extend(
            trace_attachment_ids
                .into_iter()
                .map(|trace_id| AssetUsage::TraceAttachment { trace_id }),
        );

        let document_cover_ids = documents::table
            .filter(documents::cover_image_asset_id.eq(Some(asset_id)))
            .select(documents::id)
            .load::<Uuid>(conn)?;
        usages.extend(
            document_cover_ids
                .into_iter()
                .map(|document_id| AssetUsage::DocumentCover { document_id }),
        );

        let document_asset_ids = documents::table
            .filter(documents::asset_id.eq(Some(asset_id)))
            .select(documents::id)
            .load::<Uuid>(conn)?;
        usages.extend(
            document_asset_ids
                .into_iter()
                .map(|document_id| AssetUsage::DocumentContentAsset { document_id }),
        );

        let album_cover_ids = albums::table
            .filter(albums::cover_asset_id.eq(Some(asset_id)))
            .select(albums::id)
            .load::<Uuid>(conn)?;
        usages.extend(
            album_cover_ids
                .into_iter()
                .map(|album_id| AssetUsage::AlbumCover { album_id }),
        );

        Ok(usages)
    }

    fn usage_can_user_read(
        usage: AssetUsage,
        viewer_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        match usage {
            AssetUsage::UserProfile => Ok(true),
            AssetUsage::TraceImage { trace_id } | AssetUsage::TraceAttachment { trace_id } => {
                let trace = Trace::find_full_trace(trace_id, pool)?;
                trace.user_can_read(viewer_user_id, pool)
            }
            AssetUsage::DocumentCover { document_id }
            | AssetUsage::DocumentContentAsset { document_id } => {
                let document = Document::find_full(document_id, pool)?;
                Ok(document.user_can_read(viewer_user_id))
            }
            AssetUsage::AlbumCover { album_id } => {
                let album = Album::find(album_id, pool)?;
                album.user_can_read(viewer_user_id, pool)
            }
        }
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<Asset, PpdcError> {
        let mut conn = pool.get()?;
        let row = assets::table
            .filter(assets::id.eq(id))
            .select(select_asset_columns())
            .first::<AssetTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_asset)
            .transpose()?
            .ok_or_else(|| PpdcError::new(404, ErrorType::ApiError, "Asset not found".to_string()))
    }

    pub fn can_user_read(
        asset_id: Uuid,
        viewer_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let asset = Asset::find(asset_id, pool)?;
        if asset.owner_user_id == viewer_user_id {
            return Ok(true);
        }

        let mut conn = pool.get()?;

        let usages = Self::find_usages_with_conn(asset_id, &mut conn)?;
        for usage in usages {
            if Self::usage_can_user_read(usage, viewer_user_id, pool)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub async fn signed_read_url(
        &self,
        expires_in_seconds: u64,
    ) -> Result<(String, NaiveDateTime), PpdcError> {
        let signer = CredentialsBuilder::default()
            .build_signer()
            .map_err(|err| {
                PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!("Failed to build GCP signer: {}", err),
                )
            })?;

        let url = SignedUrlBuilder::for_object(
            format!("projects/_/buckets/{}", self.bucket),
            self.object_key.clone(),
        )
        .with_method(Method::GET)
        .with_expiration(Duration::from_secs(expires_in_seconds))
        .sign_with(&signer)
        .await
        .map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Failed to sign asset URL: {}", err),
            )
        })?;

        let expires_at =
            Utc::now().naive_utc() + ChronoDuration::seconds(expires_in_seconds as i64);
        Ok((url, expires_at))
    }
}

impl NewAsset {
    fn create(self, pool: &DbPool) -> Result<Asset, PpdcError> {
        let mut conn = pool.get()?;
        let id = diesel::insert_into(assets::table)
            .values((
                assets::id.eq(self.id),
                assets::owner_user_id.eq(self.owner_user_id),
                assets::bucket.eq(self.bucket),
                assets::object_key.eq(self.object_key),
                assets::mime_type.eq(self.mime_type),
                assets::original_filename.eq(self.original_filename),
                assets::size_bytes.eq(self.size_bytes),
                assets::status.eq(self.status.to_db()),
            ))
            .returning(assets::id)
            .get_result::<Uuid>(&mut conn)?;
        Asset::find(id, pool)
    }
}

fn sanitize_filename(filename: &str) -> String {
    let sanitized = filename
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if sanitized.is_empty() {
        "upload.bin".to_string()
    } else {
        sanitized
    }
}

fn infer_content_type(filename: &str, explicit_content_type: Option<&str>) -> String {
    explicit_content_type
        .map(|value| value.to_string())
        .or_else(|| {
            mime_guess::from_path(filename)
                .first_raw()
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

fn validate_upload(content_type: &str, size_bytes: usize) -> Result<(), PpdcError> {
    let allowed = matches!(
        content_type,
        "image/jpeg"
            | "image/png"
            | "image/webp"
            | "image/gif"
            | "application/pdf"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "text/plain"
    );
    if !allowed {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Unsupported asset content type: {}", content_type),
        ));
    }

    let max_size_bytes = if content_type == "application/pdf" {
        25 * 1024 * 1024
    } else {
        10 * 1024 * 1024
    };
    if size_bytes > max_size_bytes {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Asset exceeds max size of {} bytes", max_size_bytes),
        ));
    }

    Ok(())
}

fn validate_image_upload(content_type: &str, size_bytes: usize) -> Result<(), PpdcError> {
    validate_upload(content_type, size_bytes)?;
    let is_image = matches!(
        content_type,
        "image/jpeg" | "image/png" | "image/webp" | "image/gif"
    );
    if !is_image {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Unsupported album cover content type: {}", content_type),
        ));
    }
    Ok(())
}

async fn upload_object_to_gcs(
    bucket_name: &str,
    object_key: &str,
    mime_type: &str,
    content_bytes: Vec<u8>,
) -> Result<(), PpdcError> {
    let storage = Storage::builder().build().await.map_err(|err| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            format!("Failed to build GCS client: {}", err),
        )
    })?;

    storage
        .write_object(
            format!("projects/_/buckets/{}", bucket_name),
            object_key.to_string(),
            Bytes::from(content_bytes),
        )
        .set_content_type(mime_type.to_string())
        .send_buffered()
        .await
        .map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Failed to upload to GCS: {}", err),
            )
        })?;

    Ok(())
}

pub async fn upload_asset_for_user(
    user_id: Uuid,
    pool: &DbPool,
    file_name: Option<String>,
    content_type: Option<String>,
    content_bytes: Vec<u8>,
) -> Result<AssetUploadResponse, PpdcError> {
    let original_filename = sanitize_filename(file_name.as_deref().unwrap_or("upload.bin"));
    if content_bytes.is_empty() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "No file provided in multipart payload".to_string(),
        ));
    }
    let mime_type = infer_content_type(&original_filename, content_type.as_deref());
    validate_upload(&mime_type, content_bytes.len())?;

    let asset_id = Uuid::new_v4();
    let bucket = crate::environment::get_gcs_bucket_name();
    let object_key = format!(
        "users/{}/original/{}/{}",
        user_id, asset_id, original_filename
    );

    upload_object_to_gcs(&bucket, &object_key, &mime_type, content_bytes.clone()).await?;

    let asset = NewAsset {
        id: asset_id,
        owner_user_id: user_id,
        bucket,
        object_key,
        mime_type,
        original_filename,
        size_bytes: content_bytes.len() as i64,
        status: AssetStatus::Ready,
    }
    .create(&pool)?;

    let (signed_url, expires_at) = asset
        .signed_read_url(crate::environment::get_assets_signed_url_ttl_seconds())
        .await?;

    Ok(AssetUploadResponse {
        asset,
        signed_url,
        expires_at,
    })
}

pub async fn upload_asset_for_user_from_multipart(
    user_id: Uuid,
    pool: &DbPool,
    mut multipart: Multipart,
) -> Result<AssetUploadResponse, PpdcError> {
    let mut file_name: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Multipart error: {}", err),
        )
    })? {
        if field.file_name().is_none() {
            continue;
        }

        file_name = field.file_name().map(|value| value.to_string());
        content_type = field.content_type().map(|value| value.to_string());
        file_bytes = Some(
            field
                .bytes()
                .await
                .map_err(|err| {
                    PpdcError::new(
                        400,
                        ErrorType::ApiError,
                        format!("Failed to read multipart field: {}", err),
                    )
                })?
                .to_vec(),
        );
        break;
    }

    let content_bytes = file_bytes.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "No file provided in multipart payload".to_string(),
        )
    })?;

    upload_asset_for_user(user_id, pool, file_name, content_type, content_bytes).await
}

pub async fn upload_image_asset_for_user(
    user_id: Uuid,
    pool: &DbPool,
    file_name: Option<String>,
    content_type: Option<String>,
    content_bytes: Vec<u8>,
) -> Result<AssetUploadResponse, PpdcError> {
    let original_filename = sanitize_filename(file_name.as_deref().unwrap_or("upload.bin"));
    if content_bytes.is_empty() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "No file provided in multipart payload".to_string(),
        ));
    }
    let mime_type = infer_content_type(&original_filename, content_type.as_deref());
    validate_image_upload(&mime_type, content_bytes.len())?;

    let asset_id = Uuid::new_v4();
    let bucket = crate::environment::get_gcs_bucket_name();
    let object_key = format!(
        "users/{}/original/{}/{}",
        user_id, asset_id, original_filename
    );

    upload_object_to_gcs(&bucket, &object_key, &mime_type, content_bytes.clone()).await?;

    let asset = NewAsset {
        id: asset_id,
        owner_user_id: user_id,
        bucket,
        object_key,
        mime_type,
        original_filename,
        size_bytes: content_bytes.len() as i64,
        status: AssetStatus::Ready,
    }
    .create(&pool)?;

    let (signed_url, expires_at) = asset
        .signed_read_url(crate::environment::get_assets_signed_url_ttl_seconds())
        .await?;

    Ok(AssetUploadResponse {
        asset,
        signed_url,
        expires_at,
    })
}

pub async fn upload_image_asset_for_user_from_multipart(
    user_id: Uuid,
    pool: &DbPool,
    mut multipart: Multipart,
) -> Result<AssetUploadResponse, PpdcError> {
    let mut file_name: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Multipart error: {}", err),
        )
    })? {
        if field.file_name().is_none() {
            continue;
        }

        file_name = field.file_name().map(|value| value.to_string());
        content_type = field.content_type().map(|value| value.to_string());
        file_bytes = Some(
            field
                .bytes()
                .await
                .map_err(|err| {
                    PpdcError::new(
                        400,
                        ErrorType::ApiError,
                        format!("Failed to read multipart field: {}", err),
                    )
                })?
                .to_vec(),
        );
        break;
    }

    let content_bytes = file_bytes.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "No file provided in multipart payload".to_string(),
        )
    })?;

    upload_image_asset_for_user(user_id, pool, file_name, content_type, content_bytes).await
}

#[debug_handler]
pub async fn post_asset_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    multipart: Multipart,
) -> Result<Json<AssetUploadResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let response = upload_asset_for_user_from_multipart(user_id, &pool, multipart).await?;
    Ok(Json(response))
}

#[debug_handler]
pub async fn get_asset_signed_url_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<AssetSignedUrlResponse>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if !Asset::can_user_read(id, viewer_user_id, &pool)? {
        return Err(PpdcError::unauthorized());
    }

    let asset = Asset::find(id, &pool)?;
    let (url, expires_at) = asset
        .signed_read_url(crate::environment::get_assets_signed_url_ttl_seconds())
        .await?;

    Ok(Json(AssetSignedUrlResponse { url, expires_at }))
}
