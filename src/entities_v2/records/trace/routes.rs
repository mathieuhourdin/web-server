use axum::{
    debug_handler,
    extract::{Extension, Json, Multipart, Path, Query},
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::cmp::Reverse;
use std::collections::HashSet;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    journal::{Journal, JournalStatus},
    journal_grant::{JournalGrant, JournalGrantScope, JournalGrantStatus},
    landscape_analysis::LandscapeAnalysis,
    lens::Lens,
    message::{
        routes::{enqueue_received_message_notification_email, is_service_mentor}, Message,
        MessageAttachment, MessageAttachmentType, MessageProcessingState, MessageType, NewMessage,
    },
    post::{model::legacy_lifecycle_for_status, NewPost, Post, PostAudienceRole, PostInteractionType, PostType},
    post_grant::{NewPostGrantDto, PostGrantScope},
    platform_infra::{
        asset::{upload_asset_for_user, upload_asset_for_user_from_multipart, AssetUploadResponse},
        mailer,
        usage_event::{create_usage_event, UsageEventType},
    },
    session::Session,
    shared::MaturingState,
    trace_attachment::{NewTraceAttachment, TraceAttachment, TraceAttachmentWithAsset},
    user::User,
};
use crate::pagination::{PaginatedResponse, PaginationParams};
use crate::work_analyzer;
use crate::entities_v2::post::routes::enqueue_post_published_notification_emails;

use super::{
    llm_qualify,
    model::{NewTrace, PatchTraceDto, Trace, UpdateTraceDto},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct TraceMessagesQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Deserialize)]
pub struct NewTraceMessageDto {
    pub recipient_user_id: Option<Uuid>,
    pub title: Option<String>,
    pub content: String,
    pub message_type: Option<MessageType>,
    pub attachment_type: Option<MessageAttachmentType>,
    pub attachment: Option<MessageAttachment>,
}

#[derive(Serialize)]
pub struct TraceMessageCreationResponse {
    pub question_message: Message,
    pub pending_reply_message: Option<Message>,
}

#[derive(Serialize)]
pub struct TraceAssetUploadResponse {
    pub trace: Trace,
    pub asset: crate::entities_v2::asset::Asset,
    pub signed_url: String,
    pub expires_at: chrono::NaiveDateTime,
}

#[derive(Serialize)]
pub struct TraceAttachmentUploadResponse {
    pub attachment: TraceAttachmentWithAsset,
    pub signed_url: String,
    pub expires_at: chrono::NaiveDateTime,
}

#[derive(Deserialize)]
pub struct CreateJournalDraftDto {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub interaction_date: Option<chrono::NaiveDateTime>,
    #[serde(default, alias = "is_ecrypted")]
    pub is_encrypted: Option<bool>,
    #[serde(default)]
    pub encryption_metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub image_asset_id: Option<Uuid>,
    #[serde(default)]
    pub timeout_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct ExtendTraceTimeoutDto {
    pub minutes: i64,
}

fn spawn_autoplay_lens_runs_for_trace(
    user_id: Uuid,
    trace_id: Uuid,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let user_lenses = Lens::get_user_lenses(user_id, pool)?;
    for lens in user_lenses.into_iter().filter(|lens| lens.autoplay) {
        let lens = if lens.target_trace_id != Some(trace_id) {
            let lens = lens.set_target_trace(Some(trace_id), pool)?;
            let _ = lens.clone().plan_pending_analyses_for_target(pool)?;
            Lens::find_full_lens(lens.id, pool)?
        } else {
            lens
        };
        tokio::spawn(async move { work_analyzer::run_lens(lens.id).await });
    }
    Ok(())
}

fn enqueue_autoplay_lens_runs_for_trace(user_id: Uuid, trace_id: Uuid, pool: DbPool) {
    tokio::spawn(async move {
        if let Err(err) = spawn_autoplay_lens_runs_for_trace(user_id, trace_id, &pool) {
            tracing::warn!(
                target: "analysis",
                "autoplay_lens_enqueue_failed trace_id={} user_id={} message={}",
                trace_id,
                user_id,
                err.message
            );
        }
    });
}

const MAX_TRACE_TIMEOUT_HOURS: i64 = 8;

fn is_trace_timeout_expired(trace: &Trace) -> bool {
    trace.status == super::enums::TraceStatus::Draft
        && trace
            .timeout_at
            .map(|timeout_at| timeout_at <= Utc::now())
            .unwrap_or(false)
}

fn validate_timeout_at(timeout_at: DateTime<Utc>) -> Result<(), PpdcError> {
    let now = Utc::now();
    if timeout_at <= now {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "timeout_at must be in the future".to_string(),
        ));
    }

    if timeout_at > now + ChronoDuration::hours(MAX_TRACE_TIMEOUT_HOURS) {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            format!(
                "timeout_at cannot be more than {} hours in the future",
                MAX_TRACE_TIMEOUT_HOURS
            ),
        ));
    }

    Ok(())
}

fn validate_timeout_extension_minutes(minutes: i64) -> Result<(), PpdcError> {
    if minutes <= 0 {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Timeout extension minutes must be greater than 0".to_string(),
        ));
    }
    Ok(())
}

fn enqueue_trace_post_finalize_side_effects(trace: &Trace, pool: &DbPool) {
    if trace.trace_type == super::enums::TraceType::UserTrace {
        if let Err(err) = ensure_default_draft_post_for_shared_trace(trace, pool) {
            tracing::warn!(
                target: "post",
                "shared_trace_default_post_create_failed trace_id={} message={}",
                trace.id,
                err.message
            );
        }
    }
}

async fn finalize_trace_transition(
    mut trace: Trace,
    pool: &DbPool,
    session_id: Option<Uuid>,
    auto_finalized: bool,
    preserve_interaction_date: bool,
) -> Result<Trace, PpdcError> {
    let timeout_at_before = trace.timeout_at;

    if !trace.is_encrypted && !trace.content.trim().is_empty() {
        let qualified = llm_qualify::qualify_trace(trace.content.as_str()).await?;
        trace.title = qualified.title;
        trace.subtitle = qualified.subtitle;
        if !preserve_interaction_date {
            if let Some(qualified_interaction_date) = qualified
                .interaction_date
                .and_then(|d| d.and_hms_opt(12, 0, 0))
            {
                trace.interaction_date = qualified_interaction_date;
            }
        }
    }

    trace.status = super::enums::TraceStatus::Finalized;
    trace.finalized_at = Some(Utc::now().naive_utc());
    trace.timeout_at = None;

    let trace = trace.update(pool)?;

    if !trace.is_encrypted {
        enqueue_autoplay_lens_runs_for_trace(trace.user_id, trace.id, pool.clone());
        enqueue_trace_post_finalize_side_effects(&trace, pool);
    }

    if auto_finalized {
        let _ = create_usage_event(
            trace.user_id,
            session_id,
            UsageEventType::TraceTimeoutAutoFinalized,
            Some(trace.id),
            Some(serde_json::json!({
                "timeout_at": timeout_at_before,
            })),
            pool,
        );
    }

    Ok(trace)
}

async fn finalize_expired_trace_if_needed(
    trace: Trace,
    pool: &DbPool,
    session_id: Option<Uuid>,
) -> Result<Trace, PpdcError> {
    if is_trace_timeout_expired(&trace) {
        finalize_trace_transition(trace, pool, session_id, true, false).await
    } else {
        Ok(trace)
    }
}

async fn finalize_expired_drafts_for_user(
    user_id: Uuid,
    pool: &DbPool,
    session_id: Option<Uuid>,
) -> Result<(), PpdcError> {
    let expired_drafts = Trace::get_expired_drafts_for_user(user_id, pool)?;
    for trace in expired_drafts {
        let _ = finalize_trace_transition(trace, pool, session_id, true, false).await?;
    }
    Ok(())
}

async fn parse_trace_attachment_multipart(
    mut multipart: Multipart,
) -> Result<(Option<String>, Option<String>, Vec<u8>, Option<String>), PpdcError> {
    let mut file_name: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut attachment_name: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        PpdcError::new(400, ErrorType::ApiError, format!("Multipart error: {}", err))
    })? {
        let field_name = field.name().map(|value| value.to_string());
        if field.file_name().is_some() {
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
            continue;
        }

        if field_name.as_deref() == Some("attachment_name") {
            attachment_name = Some(field.text().await.map_err(|err| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    format!("Failed to read attachment_name: {}", err),
                )
            })?);
        }
    }

    let file_bytes = file_bytes.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "No file provided in multipart payload".to_string(),
        )
    })?;

    Ok((file_name, content_type, file_bytes, attachment_name))
}

fn ensure_default_draft_post_for_shared_trace(
    trace: &Trace,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let Some(journal_id) = trace.journal_id else {
        return Ok(None);
    };

    let journal = Journal::find_full(journal_id, pool)?;
    if journal.is_encrypted || journal.status == JournalStatus::Archived {
        return Ok(None);
    }
    if !JournalGrant::has_active_grants_for_journal(journal.id, pool)? {
        return Ok(None);
    }

    if let Some(existing_post) = Post::find_default_for_trace(trace.id, pool)? {
        return Ok(Some(existing_post.id));
    }

    let post = NewPost {
        source_trace_id: Some(trace.id),
        title: trace.title.clone(),
        subtitle: trace.subtitle.clone(),
        content: trace.content.clone(),
        image_url: None,
        image_asset_id: trace.image_asset_id,
        post_type: PostType::Idea,
        interaction_type: PostInteractionType::Output,
        user_id: trace.user_id,
        publishing_date: None,
        status: crate::entities_v2::post::PostStatus::Draft,
        audience_role: PostAudienceRole::Default,
        publishing_state: "pbsh".to_string(),
        maturing_state: MaturingState::Draft,
    }
    .create(pool)?;

    let journal_grants = JournalGrant::find_for_journal(journal.id, pool)?;
    for grant in journal_grants
        .into_iter()
        .filter(|grant| grant.status == JournalGrantStatus::Active)
    {
        let payload = NewPostGrantDto {
            grantee_user_id: grant.grantee_user_id,
            grantee_scope: match grant.grantee_scope {
                Some(JournalGrantScope::AllAcceptedFollowers) => {
                    Some(PostGrantScope::AllAcceptedFollowers)
                }
                Some(JournalGrantScope::AllPlatformUsers) => {
                    Some(PostGrantScope::AllPlatformUsers)
                }
                None => None,
            },
            access_level: None,
        };
        let _ = crate::entities_v2::post_grant::PostGrant::create_or_update(
            &post,
            trace.user_id,
            payload,
            pool,
        )?;
    }

    Ok(Some(post.id))
}

fn publish_default_post_for_trace(trace: &Trace, pool: &DbPool) -> Result<Option<Post>, PpdcError> {
    let Some(_) = ensure_default_draft_post_for_shared_trace(trace, pool)? else {
        return Ok(None);
    };

    let mut post = Post::find_default_for_trace(trace.id, pool)?.ok_or_else(|| {
        PpdcError::new(
            409,
            ErrorType::ApiError,
            "Default post could not be found for this trace".to_string(),
        )
    })?;

    if post.status == crate::entities_v2::post::PostStatus::Archived {
        return Err(PpdcError::new(
            409,
            ErrorType::ApiError,
            "Default post is archived and cannot be auto-published".to_string(),
        ));
    }

    if post.status == crate::entities_v2::post::PostStatus::Published {
        return Ok(Some(post));
    }

    post.status = crate::entities_v2::post::PostStatus::Published;
    if post.publishing_date.is_none() {
        post.publishing_date = Some(Utc::now().naive_utc());
    }
    let (publishing_state, maturing_state) = legacy_lifecycle_for_status(post.status);
    post.publishing_state = publishing_state;
    post.maturing_state = maturing_state;
    let post = post.update(pool)?;

    match enqueue_post_published_notification_emails(&post, pool) {
        Ok(email_ids) if !email_ids.is_empty() => {
            let pool_for_task = pool.clone();
            tokio::spawn(async move {
                let _ = mailer::process_pending_emails(email_ids, &pool_for_task).await;
            });
        }
        Ok(_) => {}
        Err(err) => {
            tracing::warn!(
                target: "mailer",
                "post_publish_email_enqueue_failed post_id={} message={}",
                post.id,
                err.message
            );
        }
    }

    Ok(Some(post))
}

async fn create_or_get_journal_draft(
    journal: Journal,
    user_id: Uuid,
    session_id: Option<Uuid>,
    payload: CreateJournalDraftDto,
    pool: &DbPool,
) -> Result<Trace, PpdcError> {
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    if journal.status == JournalStatus::Archived {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Cannot create a draft in an archived journal".to_string(),
        ));
    }

    if let Some(existing_draft) = Trace::find_draft_for_journal(journal.id, &pool)? {
        let existing_draft = finalize_expired_trace_if_needed(existing_draft, pool, session_id).await?;
        if existing_draft.status == super::enums::TraceStatus::Draft {
            return Ok(existing_draft);
        }
    }

    let CreateJournalDraftDto {
        content,
        interaction_date,
        is_encrypted,
        encryption_metadata,
        image_asset_id,
        timeout_at,
    } = payload;

    let interaction_date = interaction_date.unwrap_or_else(|| Utc::now().naive_utc());

    let mut trace = NewTrace::new(
        String::new(),
        String::new(),
        content,
        interaction_date,
        user_id,
        journal.id,
    );
    trace.is_encrypted = is_encrypted.unwrap_or(false);
    trace.encryption_metadata = encryption_metadata;
    trace.image_asset_id = image_asset_id;
    trace.timeout_at = timeout_at;
    if trace.is_encrypted && trace.encryption_metadata.is_none() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "encryption_metadata is required when is_encrypted is true".to_string(),
        ));
    }
    if !trace.is_encrypted {
        trace.encryption_metadata = None;
    }
    if let Some(timeout_at) = trace.timeout_at {
        validate_timeout_at(timeout_at)?;
    }
    let trace = trace.create(pool)?;
    if let Some(timeout_at) = trace.timeout_at {
        let _ = create_usage_event(
            user_id,
            session_id,
            UsageEventType::TraceTimeoutSet,
            Some(trace.id),
            Some(serde_json::json!({
                "timeout_at": timeout_at,
            })),
            pool,
        );
    }
    Ok(trace)
}

#[debug_handler]
pub async fn post_journal_draft_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    Json(payload): Json<CreateJournalDraftDto>,
) -> Result<Json<Trace>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    let trace = create_or_get_journal_draft(journal, user_id, Some(session.id), payload, &pool).await?;
    Ok(Json(trace))
}

#[debug_handler]
pub async fn get_journal_draft_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
) -> Result<Json<Option<Trace>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let draft = match Trace::find_draft_for_journal(journal_id, &pool)? {
        Some(draft) => {
            let draft = finalize_expired_trace_if_needed(draft, &pool, Some(session.id)).await?;
            if draft.status == super::enums::TraceStatus::Draft {
                Some(draft)
            } else {
                None
            }
        }
        None => None,
    };
    Ok(Json(draft))
}

#[debug_handler]
pub async fn put_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTraceDto>,
) -> Result<Json<Trace>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    trace = finalize_expired_trace_if_needed(trace, &pool, Some(session.id)).await?;

    let content_changed = payload
        .content
        .as_ref()
        .map(|content| content != &trace.content)
        .unwrap_or(false);
    let interaction_date_changed = payload
        .interaction_date
        .map(|interaction_date| interaction_date != trace.interaction_date)
        .unwrap_or(false);
    let image_asset_changed = payload
        .image_asset_id
        .map(|image_asset_id| image_asset_id != trace.image_asset_id)
        .unwrap_or(false);
    let timeout_changed = payload
        .timeout_at
        .map(|timeout_at| timeout_at != trace.timeout_at)
        .unwrap_or(false);
    let has_explicit_interaction_date = payload.interaction_date.is_some();
    let publish_default_post = payload.publish_default_post.unwrap_or(false);

    match trace.status {
        super::enums::TraceStatus::Draft => {
            if let Some(content) = payload.content {
                trace.content = content;
            }
            if let Some(interaction_date) = payload.interaction_date {
                trace.interaction_date = interaction_date;
            }
            if let Some(image_asset_id) = payload.image_asset_id {
                trace.image_asset_id = image_asset_id;
            }
            if let Some(timeout_at) = payload.timeout_at {
                trace.timeout_at = timeout_at;
            }
            if let Some(timeout_at) = trace.timeout_at {
                validate_timeout_at(timeout_at)?;
            }

            if let Some(status) = payload.status {
                match status {
                    super::enums::TraceStatus::Draft => {}
                    super::enums::TraceStatus::Finalized => {
                        if trace.content.trim().is_empty() {
                            return Err(PpdcError::new(
                                400,
                                ErrorType::ApiError,
                                "Cannot finalize an empty trace".to_string(),
                            ));
                        }
                        let trace =
                            finalize_trace_transition(
                                trace,
                                &pool,
                                Some(session.id),
                                false,
                                has_explicit_interaction_date,
                            )
                            .await?;
                        if publish_default_post {
                            let _ = publish_default_post_for_trace(&trace, &pool)?;
                        }
                        return Ok(Json(trace));
                    }
                    super::enums::TraceStatus::Archived => {
                        trace.status = super::enums::TraceStatus::Archived;
                        trace.timeout_at = None;
                    }
                }
            }
        }
        super::enums::TraceStatus::Finalized => {
            if content_changed || interaction_date_changed || image_asset_changed || timeout_changed {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Cannot update content, interaction_date, image_asset_id or timeout_at once trace is finalized".to_string(),
                ));
            }

            match payload.status {
                Some(super::enums::TraceStatus::Archived) => {
                    trace.status = super::enums::TraceStatus::Archived;
                    trace.timeout_at = None;
                }
                Some(super::enums::TraceStatus::Draft) => {
                    return Err(PpdcError::new(
                        400,
                        ErrorType::ApiError,
                        "Cannot move a finalized trace back to draft".to_string(),
                    ));
                }
                Some(super::enums::TraceStatus::Finalized) | None => {}
            }
        }
        super::enums::TraceStatus::Archived => {
            if content_changed || interaction_date_changed || image_asset_changed || timeout_changed {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Cannot update content, interaction_date, image_asset_id or timeout_at once trace is archived".to_string(),
                ));
            }

            match payload.status {
                Some(super::enums::TraceStatus::Archived) => {
                    trace.timeout_at = None;
                }
                None => {}
                Some(super::enums::TraceStatus::Draft) => {
                    return Err(PpdcError::new(
                        400,
                        ErrorType::ApiError,
                        "Cannot move an archived trace back to draft".to_string(),
                    ));
                }
                Some(super::enums::TraceStatus::Finalized) => {
                    return Err(PpdcError::new(
                        400,
                        ErrorType::ApiError,
                        "Cannot finalize an archived trace".to_string(),
                    ));
                }
            }
        }
    }

    let trace = trace.update(&pool)?;
    if publish_default_post {
        if trace.status != super::enums::TraceStatus::Finalized {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "publish_default_post requires the trace to be finalized".to_string(),
            ));
        }
        let _ = publish_default_post_for_trace(&trace, &pool)?;
    }
    if timeout_changed {
        if let Some(Some(timeout_at)) = payload.timeout_at {
            let _ = create_usage_event(
                user_id,
                Some(session.id),
                UsageEventType::TraceTimeoutSet,
                Some(trace.id),
                Some(serde_json::json!({
                    "timeout_at": timeout_at,
                })),
                &pool,
            );
        }
    }

    Ok(Json(trace))
}

#[debug_handler]
pub async fn patch_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<PatchTraceDto>,
) -> Result<Json<Trace>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    trace = finalize_expired_trace_if_needed(trace, &pool, Some(session.id)).await?;
    if trace.status != super::enums::TraceStatus::Draft {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Only draft traces can be patched".to_string(),
        ));
    }

    if let Some(content) = payload.content {
        trace.content = content;
    }
    if let Some(interaction_date) = payload.interaction_date {
        trace.interaction_date = interaction_date;
    }
    if let Some(image_asset_id) = payload.image_asset_id {
        trace.image_asset_id = image_asset_id;
    }
    if let Some(timeout_at) = payload.timeout_at {
        trace.timeout_at = timeout_at;
    }
    if let Some(timeout_at) = trace.timeout_at {
        validate_timeout_at(timeout_at)?;
    }

    let trace = trace.update(&pool)?;
    if let Some(Some(timeout_at)) = payload.timeout_at {
        let _ = create_usage_event(
            user_id,
            Some(session.id),
            UsageEventType::TraceTimeoutSet,
            Some(trace.id),
            Some(serde_json::json!({
                "timeout_at": timeout_at,
            })),
            &pool,
        );
    }
    Ok(Json(trace))
}

#[debug_handler]
pub async fn post_trace_asset_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<TraceAssetUploadResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    trace = finalize_expired_trace_if_needed(trace, &pool, Some(session.id)).await?;
    if trace.status != super::enums::TraceStatus::Draft {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Only draft traces can receive uploaded assets".to_string(),
        ));
    }

    let AssetUploadResponse {
        asset,
        signed_url,
        expires_at,
    } = upload_asset_for_user_from_multipart(user_id, &pool, multipart).await?;

    trace.image_asset_id = Some(asset.id);
    let trace = trace.update(&pool)?;

    Ok(Json(TraceAssetUploadResponse {
        trace,
        asset,
        signed_url,
        expires_at,
    }))
}

#[debug_handler]
pub async fn get_trace_attachments_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TraceAttachmentWithAsset>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let attachments = TraceAttachment::find_with_assets_for_trace(id, &pool)?;
    Ok(Json(attachments))
}

#[debug_handler]
pub async fn post_trace_attachment_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<TraceAttachmentUploadResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    trace = finalize_expired_trace_if_needed(trace, &pool, Some(session.id)).await?;
    if trace.status != super::enums::TraceStatus::Draft {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Only draft traces can receive attachment uploads".to_string(),
        ));
    }

    let (file_name, content_type, file_bytes, attachment_name) =
        parse_trace_attachment_multipart(multipart).await?;
    let AssetUploadResponse {
        asset,
        signed_url,
        expires_at,
    } = upload_asset_for_user(user_id, &pool, file_name.clone(), content_type, file_bytes).await?;
    let attachment_name = attachment_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            file_name
                .unwrap_or_else(|| asset.original_filename.clone())
                .trim()
                .to_string()
        });

    let attachment = NewTraceAttachment {
        trace_id: trace.id,
        asset_id: asset.id,
        attachment_name,
    }
    .create(&pool)?;
    let attachment = TraceAttachmentWithAsset {
        id: attachment.id,
        trace_id: attachment.trace_id,
        asset_id: attachment.asset_id,
        attachment_name: attachment.attachment_name,
        created_at: attachment.created_at,
        asset,
    };

    Ok(Json(TraceAttachmentUploadResponse {
        attachment,
        signed_url,
        expires_at,
    }))
}

#[debug_handler]
pub async fn delete_trace_attachment_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((trace_id, attachment_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<TraceAttachment>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    trace = finalize_expired_trace_if_needed(trace, &pool, Some(session.id)).await?;
    if trace.status != super::enums::TraceStatus::Draft {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Only draft traces can remove attachments".to_string(),
        ));
    }

    let attachment = TraceAttachment::find(attachment_id, &pool)?;
    if attachment.trace_id != trace_id {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "Trace attachment not found".to_string(),
        ));
    }
    TraceAttachment::delete(attachment_id, &pool)?;
    Ok(Json(attachment))
}

#[debug_handler]
pub async fn post_trace_extend_timeout_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ExtendTraceTimeoutDto>,
) -> Result<Json<Trace>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    validate_timeout_extension_minutes(payload.minutes)?;

    let mut trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    trace = finalize_expired_trace_if_needed(trace, &pool, Some(session.id)).await?;
    if trace.status != super::enums::TraceStatus::Draft {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Only draft traces can extend timeout".to_string(),
        ));
    }

    let previous_timeout_at = trace.timeout_at.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "Trace has no timeout to extend".to_string(),
        )
    })?;
    let new_timeout_at = previous_timeout_at + ChronoDuration::minutes(payload.minutes);
    validate_timeout_at(new_timeout_at)?;

    trace.timeout_at = Some(new_timeout_at);
    let trace = trace.update(&pool)?;

    let _ = create_usage_event(
        user_id,
        Some(session.id),
        UsageEventType::TraceTimeoutExtended,
        Some(trace.id),
        Some(serde_json::json!({
            "previous_timeout_at": previous_timeout_at,
            "new_timeout_at": new_timeout_at,
            "minutes": payload.minutes,
        })),
        &pool,
    );

    Ok(Json(trace))
}

#[debug_handler]
pub async fn get_all_traces_for_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Trace>>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if session_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    finalize_expired_drafts_for_user(user_id, &pool, Some(session.id)).await?;
    let pagination = params.validate()?;
    let (traces, total) =
        Trace::get_all_for_user_paginated(user_id, pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(traces, pagination, total)))
}

#[debug_handler]
pub async fn get_trace_drafts_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Trace>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    finalize_expired_drafts_for_user(user_id, &pool, Some(session.id)).await?;
    let pagination = params.validate()?;
    let (drafts, total) = Trace::get_non_empty_drafts_for_user_paginated(
        user_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(drafts, pagination, total)))
}

#[debug_handler]
pub async fn get_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Trace>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != session_user_id {
        return Err(PpdcError::unauthorized());
    }
    let trace = finalize_expired_trace_if_needed(trace, &pool, Some(session.id)).await?;
    Ok(Json(trace))
}

#[debug_handler]
pub async fn get_trace_analysis_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
) -> Result<Json<Option<LandscapeAnalysis>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let user = User::find(&user_id, &pool)?;
    let current_lens_id = user.current_lens_id.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "current_lens_id is not set for user".to_string(),
        )
    })?;

    let lens = Lens::find_full_lens(current_lens_id, &pool)?;
    if lens.user_id != Some(user_id) {
        return Err(PpdcError::unauthorized());
    }
    let analysis_scope_ids = lens
        .get_analysis_scope_ids(&pool)?
        .into_iter()
        .collect::<HashSet<_>>();

    let mut analyses = LandscapeAnalysis::find_by_trace(trace_id, &pool)?
        .into_iter()
        .filter(|analysis| analysis_scope_ids.contains(&analysis.id))
        .collect::<Vec<_>>();
    analyses
        .sort_by_key(|analysis| Reverse(analysis.interaction_date.unwrap_or(analysis.created_at)));

    Ok(Json(analyses.into_iter().next()))
}

#[debug_handler]
pub async fn get_trace_messages_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
    Query(params): Query<TraceMessagesQuery>,
) -> Result<Json<PaginatedResponse<Message>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let pagination = params.pagination.validate()?;
    let (messages, total) = Message::find_for_trace_conversation_paginated(
        user_id,
        trace_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(messages, pagination, total)))
}

#[debug_handler]
pub async fn post_trace_message_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
    Json(payload): Json<NewTraceMessageDto>,
) -> Result<Json<TraceMessageCreationResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    let sender_is_owner = trace.user_id == user_id;
    if !sender_is_owner {
        return Err(PpdcError::unauthorized());
    }
    let recipient = payload
        .recipient_user_id
        .map(|recipient_user_id| User::find(&recipient_user_id, &pool))
        .transpose()?;
    let recipient_is_service_mentor = match recipient.as_ref() {
        Some(recipient) => is_service_mentor(recipient, &pool)?,
        None => false,
    };
    let message_type = match payload.message_type {
        Some(MessageType::General) if recipient_is_service_mentor => MessageType::Question,
        Some(message_type) => message_type,
        None if recipient_is_service_mentor => MessageType::Question,
        None => MessageType::General,
    };

    if matches!(
        message_type,
        MessageType::Question | MessageType::TarotReadingRequest
    ) {
        if !sender_is_owner {
            return Err(PpdcError::new(
                401,
                ErrorType::ApiError,
                "Only the trace owner can send mentor requests for this trace".to_string(),
            ));
        }

        let mentor = recipient.ok_or_else(|| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                "recipient_user_id is required for mentor requests".to_string(),
            )
        })?;
        if !is_service_mentor(&mentor, &pool)? {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "recipient_user_id must belong to a service mentor for mentor requests"
                    .to_string(),
            ));
        }

        if message_type == MessageType::TarotReadingRequest {
            let has_tarot_attachment = matches!(
                (payload.attachment_type, payload.attachment.as_ref()),
                (
                    Some(MessageAttachmentType::TarotReading),
                    Some(MessageAttachment::TarotReading(_))
                )
            );
            if !has_tarot_attachment {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "tarot_reading_request requires attachment_type=tarot_reading and a tarot attachment payload".to_string(),
                ));
            }
        }

        let question_message = NewMessage {
            sender_user_id: user_id,
            recipient_user_id: mentor.id,
            landscape_analysis_id: None,
            trace_id: Some(trace_id),
            post_id: None,
            reply_to_message_id: None,
            message_type,
            processing_state: MessageProcessingState::Processed,
            title: payload.title.unwrap_or_default(),
            content: payload.content,
            attachment_type: payload.attachment_type,
            attachment: payload.attachment,
        }
        .create(&pool)?;

        let pending_reply_message = NewMessage {
            sender_user_id: mentor.id,
            recipient_user_id: user_id,
            landscape_analysis_id: None,
            trace_id: Some(trace_id),
            post_id: None,
            reply_to_message_id: Some(question_message.id),
            message_type: MessageType::MentorReply,
            processing_state: MessageProcessingState::Pending,
            title: String::new(),
            content: String::new(),
            attachment_type: None,
            attachment: None,
        }
        .create(&pool)?;

        let pool_for_task = pool.clone();
        let pending_reply_id = pending_reply_message.id;
        tokio::spawn(async move {
            let _ = work_analyzer::run_message(pending_reply_id, &pool_for_task).await;
        });

        return Ok(Json(TraceMessageCreationResponse {
            question_message,
            pending_reply_message: Some(pending_reply_message),
        }));
    }

    let journal_id = trace.journal_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;

    let recipient_user_id = if sender_is_owner {
        let recipient_user_id = payload.recipient_user_id.ok_or_else(|| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                "recipient_user_id is required when the trace owner sends a user message"
                    .to_string(),
            )
        })?;
        if recipient_user_id == user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot send a trace message to yourself".to_string(),
            ));
        }
        if !JournalGrant::user_can_read_journal(&journal, recipient_user_id, &pool)? {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Recipient must currently have access to the trace journal".to_string(),
            ));
        }
        recipient_user_id
    } else {
        if !JournalGrant::user_can_read_journal(&journal, user_id, &pool)? {
            return Err(PpdcError::unauthorized());
        }
        trace.user_id
    };

    let message = NewMessage {
        sender_user_id: user_id,
        recipient_user_id,
        landscape_analysis_id: None,
        trace_id: Some(trace_id),
        post_id: None,
        reply_to_message_id: None,
        message_type,
        processing_state: MessageProcessingState::Processed,
        title: payload.title.unwrap_or_default(),
        content: payload.content,
        attachment_type: payload.attachment_type,
        attachment: payload.attachment,
    }
    .create(&pool)?;
    if let Some(email_id) = enqueue_received_message_notification_email(&message, &pool)? {
        let pool_for_task = pool.clone();
        tokio::spawn(async move {
            let _ = mailer::process_pending_emails(vec![email_id], &pool_for_task).await;
        });
    }

    Ok(Json(TraceMessageCreationResponse {
        question_message: message,
        pending_reply_message: None,
    }))
}

#[debug_handler]
pub async fn get_traces_for_journal_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Trace>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let pagination = params.validate()?;
    let (traces, total) =
        Trace::get_all_for_journal_paginated(id, pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(traces, pagination, total)))
}
