use std::collections::HashMap;

use google_cloud_auth::credentials::Builder as CredentialsBuilder;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    asset::Asset,
    device::Device,
    error::{ErrorType, PpdcError},
    message::Message,
    post::Post,
    source_projection::{load_source_projection_map, SourceProjectionKind},
    user::User,
};

const FIREBASE_MESSAGING_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";

#[derive(Debug, Clone)]
pub struct PushNotification {
    pub data: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct PushDispatchResult {
    pub attempted_count: usize,
    pub sent_count: usize,
}

impl PushDispatchResult {
    pub fn any_sent(&self) -> bool {
        self.sent_count > 0
    }
}

#[derive(Debug, Serialize)]
struct FcmRequest<'a> {
    message: FcmMessage<'a>,
}

#[derive(Debug, Serialize)]
struct FcmMessage<'a> {
    token: &'a str,
    data: &'a HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct FcmErrorResponse {
    error: Option<FcmError>,
}

#[derive(Debug, Deserialize)]
struct FcmError {
    status: Option<String>,
    message: Option<String>,
}

async fn firebase_access_token() -> Result<String, PpdcError> {
    let credentials = CredentialsBuilder::default()
        .with_scopes([FIREBASE_MESSAGING_SCOPE])
        .build_access_token_credentials()
        .map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Failed to build Firebase credentials: {}", err),
            )
        })?;

    let token = credentials.access_token().await.map_err(|err| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            format!("Failed to get Firebase access token: {}", err),
        )
    })?;
    Ok(token.token)
}

fn is_invalid_fcm_token(status: Option<&str>, message: Option<&str>) -> bool {
    matches!(
        status,
        Some("NOT_FOUND") | Some("INVALID_ARGUMENT") | Some("UNREGISTERED")
    ) || message
        .map(|value| {
            value.contains("registration token is not a valid")
                || value.contains("Requested entity was not found")
                || value.contains("UNREGISTERED")
        })
        .unwrap_or(false)
}

async fn send_fcm_to_device(
    device: &Device,
    notification: &PushNotification,
    access_token: &str,
    project_id: &str,
    pool: &DbPool,
) -> Result<bool, PpdcError> {
    let Some(push_token) = device.push_token.as_deref() else {
        return Ok(false);
    };
    let url = format!(
        "https://fcm.googleapis.com/v1/projects/{}/messages:send",
        project_id
    );
    let payload = FcmRequest {
        message: FcmMessage {
            token: push_token,
            data: &notification.data,
        },
    };

    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(access_token)
        .json(&payload)
        .send()
        .await
        .map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Failed to send FCM push: {}", err),
            )
        })?;

    if response.status().is_success() {
        info!(
            target: "push",
            device_id = %device.id,
            user_id = %device.user_id,
            "fcm_push_sent"
        );
        return Ok(true);
    }

    let status_code = response.status();
    let response_text = response.text().await.unwrap_or_default();
    let fcm_error = serde_json::from_str::<FcmErrorResponse>(&response_text).ok();
    let status = fcm_error
        .as_ref()
        .and_then(|error| error.error.as_ref())
        .and_then(|error| error.status.as_deref());
    let message = fcm_error
        .as_ref()
        .and_then(|error| error.error.as_ref())
        .and_then(|error| error.message.as_deref());

    warn!(
        target: "push",
        device_id = %device.id,
        user_id = %device.user_id,
        status_code = %status_code,
        fcm_status = status.unwrap_or(""),
        fcm_message = message.unwrap_or(""),
        "fcm_push_failed"
    );

    if is_invalid_fcm_token(status, message) {
        Device::clear_push_token(device.id, pool)?;
    }

    if status_code == StatusCode::UNAUTHORIZED || status_code == StatusCode::FORBIDDEN {
        return Err(PpdcError::new(
            500,
            ErrorType::InternalError,
            format!("FCM authorization failed: {}", response_text),
        ));
    }

    Ok(false)
}

pub async fn send_to_user(
    user_id: Uuid,
    notification: PushNotification,
    pool: &DbPool,
) -> Result<PushDispatchResult, PpdcError> {
    let devices = Device::find_active_fcm_targets_for_user(user_id, pool)?;
    send_to_devices(devices, notification, pool).await
}

pub async fn send_to_mobile_user(
    user_id: Uuid,
    notification: PushNotification,
    pool: &DbPool,
) -> Result<PushDispatchResult, PpdcError> {
    let devices = Device::find_active_mobile_fcm_targets_for_user(user_id, pool)?;
    send_to_devices(devices, notification, pool).await
}

async fn send_to_devices(
    devices: Vec<Device>,
    notification: PushNotification,
    pool: &DbPool,
) -> Result<PushDispatchResult, PpdcError> {
    let mut result = PushDispatchResult {
        attempted_count: devices.len(),
        sent_count: 0,
    };
    if devices.is_empty() {
        return Ok(result);
    }

    let project_id = crate::environment::get_firebase_project_id();
    let access_token = firebase_access_token().await?;
    let mut fatal_error: Option<PpdcError> = None;

    for device in devices {
        match send_fcm_to_device(&device, &notification, &access_token, &project_id, pool).await {
            Ok(true) => result.sent_count += 1,
            Ok(false) => {}
            Err(err) => {
                fatal_error = Some(err);
                break;
            }
        }
    }

    if result.any_sent() {
        return Ok(result);
    }
    if let Some(err) = fatal_error {
        return Err(err);
    }
    Ok(result)
}

async fn sender_avatar_url(sender: &User, pool: &DbPool) -> Option<String> {
    if let Some(asset_id) = sender.profile_picture_asset_id {
        match Asset::find(asset_id, pool) {
            Ok(asset) => match asset
                .signed_read_url(crate::environment::get_assets_signed_url_ttl_seconds())
                .await
            {
                Ok((url, _expires_at)) => return Some(url),
                Err(err) => {
                    warn!(
                        target: "push",
                        sender_user_id = %sender.id,
                        asset_id = %asset_id,
                        error = %err.message,
                        "sender_avatar_signed_url_failed"
                    );
                }
            },
            Err(err) => {
                warn!(
                    target: "push",
                    sender_user_id = %sender.id,
                    asset_id = %asset_id,
                    error = %err.message,
                    "sender_avatar_asset_lookup_failed"
                );
            }
        }
    }

    sender
        .profile_picture_url
        .as_ref()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

pub(crate) async fn message_received_notification(
    message: &Message,
    pool: &DbPool,
) -> Result<PushNotification, PpdcError> {
    let sender = User::find(&message.sender_user_id, pool)?;
    let mut data = HashMap::new();
    data.insert("event_type".to_string(), "message_received".to_string());
    data.insert(
        "sender_user_id".to_string(),
        message.sender_user_id.to_string(),
    );
    data.insert("sender_display_name".to_string(), sender.display_name());
    if let Some(avatar_url) = sender_avatar_url(&sender, pool).await {
        data.insert("sender_avatar_url".to_string(), avatar_url);
    }
    data.insert("message_id".to_string(), message.id.to_string());
    data.insert("message_content".to_string(), message.content.clone());
    data.insert(
        "message_timestamp".to_string(),
        message.created_at.and_utc().timestamp_millis().to_string(),
    );

    Ok(PushNotification { data })
}

fn source_kind_value(source_kind: SourceProjectionKind) -> &'static str {
    match source_kind {
        SourceProjectionKind::Trace => "trace",
        SourceProjectionKind::Document => "document",
        SourceProjectionKind::Album => "album",
    }
}

fn content_preview(value: &str) -> String {
    const MAX_CHARS: usize = 280;
    let trimmed = value.trim();
    let mut preview = trimmed.chars().take(MAX_CHARS).collect::<String>();
    if trimmed.chars().count() > MAX_CHARS {
        preview.push_str("...");
    }
    preview
}

async fn signed_asset_url(asset_id: Uuid, pool: &DbPool, target: &'static str) -> Option<String> {
    match Asset::find(asset_id, pool) {
        Ok(asset) => match asset
            .signed_read_url(crate::environment::get_assets_signed_url_ttl_seconds())
            .await
        {
            Ok((url, _expires_at)) => Some(url),
            Err(err) => {
                warn!(
                    target: "push",
                    asset_id = %asset_id,
                    error = %err.message,
                    "{}",
                    target
                );
                None
            }
        },
        Err(err) => {
            warn!(
                target: "push",
                asset_id = %asset_id,
                error = %err.message,
                "{}",
                target
            );
            None
        }
    }
}

pub(crate) async fn post_published_notification(
    post: &Post,
    pool: &DbPool,
) -> Result<Option<PushNotification>, PpdcError> {
    let Some(source_ref) = post.source_ref() else {
        return Ok(None);
    };

    let owner = User::find(&post.user_id, pool)?;
    let projection = {
        let mut conn = pool.get()?;
        let mut projections = load_source_projection_map(&[source_ref], &mut conn)?;
        projections.remove(&source_ref)
    };
    let Some(projection) = projection else {
        return Ok(None);
    };

    let mut data = HashMap::new();
    data.insert("event_type".to_string(), "post_published".to_string());
    data.insert("post_id".to_string(), post.id.to_string());
    data.insert(
        "source_kind".to_string(),
        source_kind_value(projection.source_kind).to_string(),
    );
    data.insert("source_id".to_string(), projection.source_id.to_string());
    if let Some(journal_id) = projection.journal_id {
        data.insert("journal_id".to_string(), journal_id.to_string());
    }
    data.insert("publisher_user_id".to_string(), post.user_id.to_string());
    data.insert("publisher_display_name".to_string(), owner.display_name());
    data.insert("title".to_string(), projection.title);
    data.insert(
        "content_preview".to_string(),
        content_preview(&projection.content),
    );
    data.insert(
        "published_at".to_string(),
        post.publishing_date
            .unwrap_or(post.created_at)
            .and_utc()
            .timestamp_millis()
            .to_string(),
    );
    if let Some(asset_id) = projection.cover_image_asset_id {
        if let Some(url) = signed_asset_url(asset_id, pool, "post_cover_signed_url_failed").await {
            data.insert("cover_image_url".to_string(), url);
        }
    }

    Ok(Some(PushNotification { data }))
}
