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
    journal::Journal,
    message::{Message, MessageReaction},
    post::Post,
    relationship::Relationship,
    source_projection::{load_source_projection_map, SourceProjectionKind},
    user::User,
};

const FIREBASE_MESSAGING_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";

#[derive(Debug, Clone)]
pub struct PushNotification {
    pub title: String,
    pub body: String,
    pub data: HashMap<String, String>,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SilentPush {
    pub data: HashMap<String, String>,
    pub collapse_key: String,
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
    android: FcmAndroidConfig,
    apns: FcmApnsConfig,
}

#[derive(Debug, Serialize)]
struct FcmAndroidConfig {
    priority: &'static str,
}

#[derive(Debug, Serialize)]
struct FcmApnsConfig {
    headers: FcmApnsHeaders,
    payload: FcmApnsPayload,
}

#[derive(Debug, Serialize)]
struct FcmApnsHeaders {
    #[serde(rename = "apns-push-type")]
    push_type: &'static str,
    #[serde(rename = "apns-priority")]
    priority: &'static str,
}

#[derive(Debug, Serialize)]
struct FcmApnsPayload {
    aps: FcmApsPayload,
}

#[derive(Debug, Serialize)]
struct FcmApsPayload {
    alert: FcmApnsAlert,
    sound: &'static str,
    #[serde(rename = "mutable-content")]
    mutable_content: u8,
    #[serde(rename = "thread-id", skip_serializing_if = "Option::is_none")]
    thread_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct FcmApnsAlert {
    title: String,
    body: String,
}

#[derive(Debug, Serialize)]
struct FcmSilentRequest<'a> {
    message: FcmSilentMessage<'a>,
}

#[derive(Debug, Serialize)]
struct FcmSilentMessage<'a> {
    token: &'a str,
    data: &'a HashMap<String, String>,
    android: FcmSilentAndroidConfig<'a>,
    apns: FcmSilentApnsConfig<'a>,
}

#[derive(Debug, Serialize)]
struct FcmSilentAndroidConfig<'a> {
    priority: &'static str,
    collapse_key: &'a str,
}

#[derive(Debug, Serialize)]
struct FcmSilentApnsConfig<'a> {
    headers: FcmSilentApnsHeaders<'a>,
    payload: FcmSilentApnsPayload,
}

#[derive(Debug, Serialize)]
struct FcmSilentApnsHeaders<'a> {
    #[serde(rename = "apns-push-type")]
    push_type: &'static str,
    #[serde(rename = "apns-priority")]
    priority: &'static str,
    #[serde(rename = "apns-collapse-id")]
    collapse_id: &'a str,
}

#[derive(Debug, Serialize)]
struct FcmSilentApnsPayload {
    aps: FcmSilentApsPayload,
}

#[derive(Debug, Serialize)]
struct FcmSilentApsPayload {
    #[serde(rename = "content-available")]
    content_available: u8,
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
    let payload = FcmRequest {
        message: FcmMessage {
            token: push_token,
            data: &notification.data,
            android: FcmAndroidConfig { priority: "HIGH" },
            apns: FcmApnsConfig {
                headers: FcmApnsHeaders {
                    push_type: "alert",
                    priority: "10",
                },
                payload: FcmApnsPayload {
                    aps: FcmApsPayload {
                        alert: FcmApnsAlert {
                            title: notification.title.clone(),
                            body: notification.body.clone(),
                        },
                        sound: "default",
                        mutable_content: 1,
                        thread_id: notification.thread_id.clone(),
                    },
                },
            },
        },
    };

    execute_fcm_request(device, &payload, access_token, project_id, pool).await
}

async fn send_silent_fcm_to_device(
    device: &Device,
    notification: &SilentPush,
    access_token: &str,
    project_id: &str,
    pool: &DbPool,
) -> Result<bool, PpdcError> {
    let Some(push_token) = device.push_token.as_deref() else {
        return Ok(false);
    };
    let payload = FcmSilentRequest {
        message: FcmSilentMessage {
            token: push_token,
            data: &notification.data,
            android: FcmSilentAndroidConfig {
                priority: "NORMAL",
                collapse_key: &notification.collapse_key,
            },
            apns: FcmSilentApnsConfig {
                headers: FcmSilentApnsHeaders {
                    push_type: "background",
                    priority: "5",
                    collapse_id: &notification.collapse_key,
                },
                payload: FcmSilentApnsPayload {
                    aps: FcmSilentApsPayload {
                        content_available: 1,
                    },
                },
            },
        },
    };

    execute_fcm_request(device, &payload, access_token, project_id, pool).await
}

async fn execute_fcm_request<T: Serialize>(
    device: &Device,
    payload: &T,
    access_token: &str,
    project_id: &str,
    pool: &DbPool,
) -> Result<bool, PpdcError> {
    let url = format!(
        "https://fcm.googleapis.com/v1/projects/{}/messages:send",
        project_id
    );

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

pub async fn send_silent_to_mobile_user(
    user_id: Uuid,
    notification: SilentPush,
    pool: &DbPool,
) -> Result<PushDispatchResult, PpdcError> {
    let devices = Device::find_active_mobile_fcm_targets_for_user(user_id, pool)?;
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
        match send_silent_fcm_to_device(&device, &notification, &access_token, &project_id, pool)
            .await
        {
            Ok(true) => result.sent_count += 1,
            Ok(false) => {}
            Err(error) => {
                fatal_error = Some(error);
                break;
            }
        }
    }
    if result.any_sent() {
        return Ok(result);
    }
    if let Some(error) = fatal_error {
        return Err(error);
    }
    Ok(result)
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
            Ok(asset) => {
                if let Some(public_url) = asset.public_url() {
                    return Some(public_url);
                }

                match asset
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
                }
            }
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
    data.insert(
        "message_type".to_string(),
        message.message_type.to_db().to_ascii_lowercase(),
    );
    data.insert("message_content".to_string(), message.content.clone());
    data.insert(
        "message_timestamp".to_string(),
        message.created_at.and_utc().timestamp_millis().to_string(),
    );

    Ok(PushNotification {
        title: sender.display_name(),
        body: message.content.clone(),
        data,
        thread_id: Some(message.sender_user_id.to_string()),
    })
}

pub(crate) async fn message_reaction_notification(
    message: &Message,
    reaction: &MessageReaction,
    pool: &DbPool,
) -> Result<PushNotification, PpdcError> {
    let reactor = User::find(&reaction.user_id, pool)?;
    let mut data = HashMap::new();
    data.insert(
        "event_type".to_string(),
        "message_reaction_updated".to_string(),
    );
    data.insert("message_id".to_string(), message.id.to_string());
    data.insert("emoji".to_string(), reaction.emoji.clone());
    data.insert("reactor_user_id".to_string(), reactor.id.to_string());
    data.insert("reactor_display_name".to_string(), reactor.display_name());
    if let Some(avatar_url) = sender_avatar_url(&reactor, pool).await {
        data.insert("reactor_avatar_url".to_string(), avatar_url.clone());
        // The iOS notification service extension already treats actor_avatar_url
        // as the generic avatar key for social events.
        data.insert("actor_avatar_url".to_string(), avatar_url);
    }
    data.insert(
        "message_sender_user_id".to_string(),
        message.sender_user_id.to_string(),
    );
    data.insert(
        "message_recipient_user_id".to_string(),
        message.recipient_user_id.to_string(),
    );
    data.insert(
        "message_content_preview".to_string(),
        content_preview(&message.content),
    );
    data.insert(
        "reacted_at".to_string(),
        reaction.updated_at.and_utc().timestamp_millis().to_string(),
    );

    Ok(PushNotification {
        title: reactor.display_name(),
        body: format!("a réagi {} à votre message", reaction.emoji),
        data,
        thread_id: Some(reactor.id.to_string()),
    })
}

async fn relationship_actor_data(
    event_type: &'static str,
    relationship: &Relationship,
    actor_user_id: Uuid,
    actor_key: &'static str,
    pool: &DbPool,
) -> Result<PushNotification, PpdcError> {
    let actor = User::find(&actor_user_id, pool)?;
    let mut data = HashMap::new();
    data.insert("event_type".to_string(), event_type.to_string());
    data.insert("relationship_id".to_string(), relationship.id.to_string());
    data.insert("relationship_type".to_string(), "follow".to_string());
    data.insert(actor_key.to_string(), actor.id.to_string());
    data.insert("actor_display_name".to_string(), actor.display_name());
    if let Some(avatar_url) = sender_avatar_url(&actor, pool).await {
        data.insert("actor_avatar_url".to_string(), avatar_url);
    }
    data.insert(
        "event_timestamp".to_string(),
        relationship
            .accepted_at
            .unwrap_or(relationship.updated_at)
            .and_utc()
            .timestamp_millis()
            .to_string(),
    );
    let action = match event_type {
        "follow_request_received" => "souhaite vous suivre",
        "follow_request_accepted" => "a accepté votre demande",
        _ => "Nouvelle activité",
    };
    Ok(PushNotification {
        title: actor.display_name(),
        body: action.to_string(),
        data,
        thread_id: None,
    })
}

pub(crate) async fn follow_request_received_notification(
    relationship: &Relationship,
    pool: &DbPool,
) -> Result<PushNotification, PpdcError> {
    relationship_actor_data(
        "follow_request_received",
        relationship,
        relationship.requester_user_id,
        "requester_user_id",
        pool,
    )
    .await
}

pub(crate) async fn follow_request_accepted_notification(
    relationship: &Relationship,
    pool: &DbPool,
) -> Result<PushNotification, PpdcError> {
    relationship_actor_data(
        "follow_request_accepted",
        relationship,
        relationship.target_user_id,
        "accepter_user_id",
        pool,
    )
    .await
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
    if let Some(original_source_id) = projection.original_source_id {
        data.insert(
            "original_source_id".to_string(),
            original_source_id.to_string(),
        );
    }
    if let Some(original_author_user_id) = projection.original_author_user_id {
        data.insert(
            "original_author_user_id".to_string(),
            original_author_user_id.to_string(),
        );
        if let Ok(original_author) = User::find(&original_author_user_id, pool) {
            data.insert(
                "original_author_display_name".to_string(),
                original_author.display_name(),
            );
            if let Some(profile_picture_url) = sender_avatar_url(&original_author, pool).await {
                data.insert(
                    "original_author_profile_picture_url".to_string(),
                    profile_picture_url,
                );
            }
        }
    }
    if let Some(journal_id) = projection.journal_id {
        data.insert("journal_id".to_string(), journal_id.to_string());
        match Journal::find_full(journal_id, pool) {
            Ok(journal) => {
                data.insert("journal_title".to_string(), journal.title);
            }
            Err(err) => {
                warn!(
                    target: "push",
                    post_id = %post.id,
                    journal_id = %journal_id,
                    error = %err.message,
                    "post_published_journal_lookup_failed"
                );
            }
        }
    }
    data.insert("publisher_user_id".to_string(), post.user_id.to_string());
    data.insert("publisher_display_name".to_string(), owner.display_name());
    if let Some(profile_picture_url) = sender_avatar_url(&owner, pool).await {
        data.insert(
            "publisher_profile_picture_url".to_string(),
            profile_picture_url,
        );
    }
    data.insert("title".to_string(), projection.title.clone());
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

    let title = owner.display_name();
    let body = if projection.title.trim().is_empty() {
        "Nouvelle publication".to_string()
    } else {
        projection.title.clone()
    };

    Ok(Some(PushNotification {
        title,
        body,
        data,
        thread_id: None,
    }))
}

pub(crate) async fn trace_mention_notification(
    post: &Post,
    pool: &DbPool,
) -> Result<Option<PushNotification>, PpdcError> {
    let Some(mut notification) = post_published_notification(post, pool).await? else {
        return Ok(None);
    };
    notification
        .data
        .insert("event_type".to_string(), "trace_mention".to_string());
    if let Some(trace_id) = post.source_trace_id {
        notification
            .data
            .insert("trace_id".to_string(), trace_id.to_string());
    }
    notification
        .data
        .insert("author_user_id".to_string(), post.user_id.to_string());
    if let Some(display_name) = notification.data.get("publisher_display_name").cloned() {
        notification
            .data
            .insert("author_display_name".to_string(), display_name);
    }
    if let Some(avatar_url) = notification
        .data
        .get("publisher_profile_picture_url")
        .cloned()
    {
        notification
            .data
            .insert("author_avatar_url".to_string(), avatar_url);
    }
    notification.body = if notification.body == "Nouvelle publication" {
        "vous a mentionné dans une trace".to_string()
    } else {
        format!("vous a mentionné dans « {} »", notification.body)
    };
    notification.thread_id = Some(post.user_id.to_string());
    Ok(Some(notification))
}

pub(crate) async fn journal_history_shared_notification(
    journal: &Journal,
    owner: &User,
    shared_trace_count: usize,
    pool: &DbPool,
) -> PushNotification {
    let owner_display_name = owner.display_name();
    let mut data = HashMap::new();
    data.insert(
        "event_type".to_string(),
        "journal_history_shared".to_string(),
    );
    data.insert("journal_id".to_string(), journal.id.to_string());
    data.insert("journal_title".to_string(), journal.title.clone());
    data.insert("publisher_user_id".to_string(), owner.id.to_string());
    data.insert(
        "publisher_display_name".to_string(),
        owner_display_name.clone(),
    );
    data.insert("publisher_handle".to_string(), owner.handle.clone());
    data.insert(
        "shared_trace_count".to_string(),
        shared_trace_count.to_string(),
    );
    if let Some(profile_picture_url) = sender_avatar_url(owner, pool).await {
        data.insert(
            "publisher_profile_picture_url".to_string(),
            profile_picture_url,
        );
    }

    let trace_label = if shared_trace_count == 1 {
        "1 trace".to_string()
    } else {
        format!("{} traces", shared_trace_count)
    };
    PushNotification {
        title: owner_display_name,
        body: format!(
            "a partagé {} du journal « {} » avec vous",
            trace_label, journal.title
        ),
        data,
        thread_id: Some(journal.id.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_apns_thread_id_when_present() {
        let payload = FcmApsPayload {
            alert: FcmApnsAlert {
                title: "Sender".to_string(),
                body: "Message".to_string(),
            },
            sound: "default",
            mutable_content: 1,
            thread_id: Some("user-id".to_string()),
        };
        let value = serde_json::to_value(payload).unwrap();
        assert_eq!(value["thread-id"], "user-id");
        assert_eq!(value["mutable-content"], 1);
    }

    #[test]
    fn omits_apns_thread_id_when_absent() {
        let payload = FcmApsPayload {
            alert: FcmApnsAlert {
                title: "Title".to_string(),
                body: "Body".to_string(),
            },
            sound: "default",
            mutable_content: 1,
            thread_id: None,
        };
        let value = serde_json::to_value(payload).unwrap();
        assert!(value.get("thread-id").is_none());
        assert_eq!(value["mutable-content"], 1);
    }

    #[test]
    fn serializes_silent_apns_payload_without_visible_alert() {
        let payload = FcmSilentApnsConfig {
            headers: FcmSilentApnsHeaders {
                push_type: "background",
                priority: "5",
                collapse_id: "wal-compilation-id",
            },
            payload: FcmSilentApnsPayload {
                aps: FcmSilentApsPayload {
                    content_available: 1,
                },
            },
        };
        let value = serde_json::to_value(payload).unwrap();

        assert_eq!(value["headers"]["apns-push-type"], "background");
        assert_eq!(value["headers"]["apns-priority"], "5");
        assert_eq!(value["payload"]["aps"]["content-available"], 1);
        assert!(value["payload"]["aps"].get("alert").is_none());
        assert!(value["payload"]["aps"].get("sound").is_none());
    }
}
