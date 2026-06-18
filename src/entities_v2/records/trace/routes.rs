use axum::{
    debug_handler,
    extract::{Extension, Json, Multipart, Path, Query},
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::cmp::Reverse;
use std::collections::HashSet;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::post::routes::enqueue_post_published_notification_emails;
use crate::entities_v2::{
    document::{Document, DocumentContentSource, DocumentRole, NewDocumentDto},
    error::{ErrorType, PpdcError},
    journal::{Journal, JournalStatus, JournalType},
    journal_grant::JournalGrant,
    landscape_analysis::LandscapeAnalysis,
    lens::Lens,
    message::{
        routes::is_service_mentor, Message, MessageAttachment, MessageAttachmentType,
        MessageProcessingState, MessageType, NewMessage,
    },
    notification,
    platform_infra::{
        asset::{
            upload_asset_for_user, upload_image_asset_for_user_from_multipart, AssetUploadResponse,
        },
        mailer,
        usage_event::{create_usage_event, UsageEventType},
    },
    post::{NewPost, Post, PostAudienceRole, PostInteractionType, PostStatus, PostType},
    post_grant::PostGrant,
    session::Session,
    trace_attachment::{
        NewTraceAttachment, TraceAttachment, TraceAttachmentReadableView,
        TraceAttachmentWithDocument,
    },
    user::{ensure_user_has_any_lens, User},
    user_post_state::UserPostState,
};
use crate::pagination::{PaginatedResponse, PaginationParams};
use crate::work_analyzer;

use super::{
    enums::{TraceSharingSensitivity, TraceStatus},
    llm_qualify,
    model::{JournalTraceView, NewTrace, PatchTraceDto, Trace, TraceReadableView, UpdateTraceDto},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct TraceMessagesQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Deserialize)]
pub struct JournalTracesQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub sharing_sensitivity: Option<TraceSharingSensitivity>,
    pub status: Option<TraceStatus>,
    pub seen: Option<bool>,
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
    pub attachment: TraceAttachmentWithDocument,
    pub signed_url: String,
    pub expires_at: chrono::NaiveDateTime,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum CreateTraceDocumentAttachmentDto {
    ExistingDocument {
        document_id: Uuid,
        #[serde(default)]
        attachment_name: Option<String>,
    },
    NewDocument {
        #[serde(default)]
        attachment_name: Option<String>,
        document: NewDocumentDto,
    },
}

#[derive(Serialize)]
pub struct TraceSeenStateResponse {
    pub trace_id: Uuid,
    pub post_id: Uuid,
    pub first_seen_at: chrono::NaiveDateTime,
    pub last_seen_at: chrono::NaiveDateTime,
}

#[derive(Deserialize)]
pub struct CreateJournalDraftDto {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub derived_from_trace_id: Option<Uuid>,
    #[serde(default)]
    pub interaction_date: Option<chrono::NaiveDateTime>,
    #[serde(default, alias = "is_ecrypted")]
    pub is_encrypted: Option<bool>,
    #[serde(default)]
    pub encryption_metadata: Option<serde_json::Value>,
    #[serde(default)]
    #[serde(alias = "image_asset_id")]
    pub content_image_asset_id: Option<Uuid>,
    #[serde(default)]
    pub sharing_sensitivity: Option<super::enums::TraceSharingSensitivity>,
    #[serde(default)]
    pub timeout_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreateFinalizedTraceDocumentAttachmentDto {
    pub document_id: Uuid,
    #[serde(default)]
    pub attachment_name: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreateFinalizedJournalTraceDto {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub derived_from_trace_id: Option<Uuid>,
    #[serde(default)]
    pub interaction_date: Option<chrono::NaiveDateTime>,
    #[serde(default, alias = "is_ecrypted")]
    pub is_encrypted: Option<bool>,
    #[serde(default)]
    pub encryption_metadata: Option<serde_json::Value>,
    #[serde(default)]
    #[serde(alias = "image_asset_id")]
    pub content_image_asset_id: Option<Uuid>,
    #[serde(default)]
    pub sharing_sensitivity: Option<super::enums::TraceSharingSensitivity>,
    #[serde(default)]
    pub publish_default_post: Option<bool>,
    #[serde(default)]
    pub document_attachments: Vec<CreateFinalizedTraceDocumentAttachmentDto>,
}

#[derive(Serialize)]
pub struct CreateFinalizedJournalTraceResponse {
    pub trace: Trace,
    pub attachments: Vec<TraceAttachmentWithDocument>,
    pub post: Option<Post>,
}

#[derive(Deserialize)]
pub struct ExtendTraceTimeoutDto {
    pub minutes: i64,
    #[serde(alias = "expected_version")]
    pub expected_version_integer: Option<i32>,
}

#[derive(Deserialize)]
pub struct TraceExpectedVersionQuery {
    #[serde(alias = "expected_version")]
    #[serde(alias = "version_integer")]
    pub expected_version_integer: i32,
}

fn spawn_autoplay_lens_runs_for_trace(
    user_id: Uuid,
    trace_id: Uuid,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    ensure_user_has_any_lens(user_id, pool)?;
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

pub(crate) fn enqueue_trace_finalized_side_effects(trace: &Trace, pool: &DbPool) {
    if !trace.is_encrypted {
        enqueue_autoplay_lens_runs_for_trace(trace.user_id, trace.id, pool.clone());
        enqueue_trace_post_finalize_side_effects(trace, pool);
    }
}

#[allow(dead_code)]
fn enqueue_trace_qualification(trace: &Trace, pool: DbPool) {
    let trace_id = trace.id;
    let trace_content = trace.content.clone();
    tokio::spawn(async move {
        match llm_qualify::qualify_trace(trace_content.as_str()).await {
            Ok(qualified) => match Trace::find_full_trace(trace_id, &pool) {
                Ok(mut trace) => {
                    trace.title = qualified.title;
                    trace.subtitle = qualified.subtitle;
                    if let Err(err) = trace.update(&pool) {
                        tracing::warn!(
                            target: "trace",
                            "trace_qualification_update_failed trace_id={} message={}",
                            trace_id,
                            err.message
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        target: "trace",
                        "trace_qualification_load_failed trace_id={} message={}",
                        trace_id,
                        err.message
                    );
                }
            },
            Err(err) => {
                tracing::warn!(
                    target: "trace",
                    "trace_qualification_failed trace_id={} message={}",
                    trace_id,
                    err.message
                );
            }
        }
    });
}

async fn finalize_trace_transition(
    mut trace: Trace,
    pool: &DbPool,
    session_id: Option<Uuid>,
    auto_finalized: bool,
    expected_version_integer: Option<i32>,
) -> Result<Trace, PpdcError> {
    let timeout_at_before = trace.timeout_at;

    trace.status = super::enums::TraceStatus::Finalized;
    trace.finalized_at = Some(Utc::now().naive_utc());
    trace.timeout_at = None;

    let trace = if let Some(expected_version_integer) = expected_version_integer {
        trace.update_with_expected_version(expected_version_integer, pool)?
    } else {
        trace.update(pool)?
    };

    if let Some(journal_id) = trace.journal_id {
        let mut journal = Journal::find_full(journal_id, pool)?;
        if journal.status == JournalStatus::Active
            && journal.journal_type == JournalType::UserJournal
            && journal.current_draft_id == Some(trace.id)
        {
            let next_draft = Trace::create_blank_draft_for_journal(&journal, pool)?;
            journal.current_draft_id = Some(next_draft.id);
            let _ = journal.update(pool)?;
        }
    }

    enqueue_trace_finalized_side_effects(&trace, pool);

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
        finalize_trace_transition(trace, pool, session_id, true, None).await
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
        let _ = finalize_trace_transition(trace, pool, session_id, true, None).await?;
    }
    Ok(())
}

fn find_published_trace_post(trace_id: Uuid, pool: &DbPool) -> Result<Option<Post>, PpdcError> {
    let Some(post) = Post::find_for_trace(trace_id, pool)? else {
        return Ok(None);
    };
    if post.status != PostStatus::Published {
        return Ok(None);
    }
    Ok(Some(post))
}

fn attach_seen_state_to_journal_trace_views(
    user_id: Uuid,
    items: Vec<JournalTraceView>,
    pool: &DbPool,
) -> Result<Vec<JournalTraceView>, PpdcError> {
    let trace_ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
    let last_seen_at_by_trace_id =
        UserPostState::find_last_seen_at_by_user_and_trace_ids(user_id, &trace_ids, pool)?;

    Ok(items
        .into_iter()
        .map(|mut item| {
            item.last_seen_at = last_seen_at_by_trace_id.get(&item.id).copied();
            item.seen = item.last_seen_at.is_some();
            item
        })
        .collect())
}

fn attach_seen_state_to_trace_readable_views(
    user_id: Uuid,
    items: Vec<TraceReadableView>,
    pool: &DbPool,
) -> Result<Vec<TraceReadableView>, PpdcError> {
    let trace_ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
    let last_seen_at_by_trace_id =
        UserPostState::find_last_seen_at_by_user_and_trace_ids(user_id, &trace_ids, pool)?;

    Ok(items
        .into_iter()
        .map(|mut item| {
            item.last_seen_at = last_seen_at_by_trace_id.get(&item.id).copied();
            item.seen = item.last_seen_at.is_some();
            item
        })
        .collect())
}

fn trace_to_readable_view(trace: Trace, include_owner_fields: bool) -> TraceReadableView {
    TraceReadableView {
        id: trace.id,
        journal_id: trace.journal_id,
        version_integer: if include_owner_fields {
            Some(trace.version_integer)
        } else {
            None
        },
        title: trace.title,
        subtitle: Some(trace.subtitle),
        content: trace.content,
        derived_from_trace_id: if include_owner_fields {
            trace.derived_from_trace_id
        } else {
            None
        },
        is_encrypted: if include_owner_fields {
            Some(trace.is_encrypted)
        } else {
            None
        },
        encryption_metadata: if include_owner_fields {
            trace.encryption_metadata
        } else {
            None
        },
        content_image_asset_id: trace.content_image_asset_id,
        sharing_sensitivity: if include_owner_fields {
            Some(trace.sharing_sensitivity)
        } else {
            None
        },
        timeout_start_at: if include_owner_fields {
            trace.timeout_start_at
        } else {
            None
        },
        timeout_at: if include_owner_fields {
            trace.timeout_at
        } else {
            None
        },
        user_id: if include_owner_fields {
            Some(trace.user_id)
        } else {
            None
        },
        trace_type: if include_owner_fields {
            Some(trace.trace_type)
        } else {
            None
        },
        status: if include_owner_fields {
            Some(trace.status)
        } else {
            None
        },
        start_writing_at: if include_owner_fields {
            Some(trace.start_writing_at)
        } else {
            None
        },
        finalized_at: trace.finalized_at,
        seen: false,
        last_seen_at: None,
        interaction_date: trace.interaction_date,
        created_at: trace.created_at,
        updated_at: trace.updated_at,
    }
}

fn require_expected_version_integer(
    expected_version_integer: Option<i32>,
) -> Result<i32, PpdcError> {
    expected_version_integer.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "expected_version_integer is required".to_string(),
        )
    })
}

async fn parse_trace_attachment_multipart(
    mut multipart: Multipart,
) -> Result<(Option<String>, Option<String>, Vec<u8>, Option<String>), PpdcError> {
    let mut file_name: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut attachment_name: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Multipart error: {}", err),
        )
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

    if let Some(existing_post) = Post::find_for_trace(trace.id, pool)? {
        return Ok(Some(existing_post.id));
    }

    let post = NewPost {
        source_trace_id: Some(trace.id),
        source_document_id: None,
        source_album_id: None,
        post_type: PostType::Idea,
        interaction_type: PostInteractionType::Output,
        user_id: trace.user_id,
        publishing_date: None,
        status: crate::entities_v2::post::PostStatus::Draft,
        audience_role: PostAudienceRole::Default,
    }
    .create(pool)?;

    Ok(Some(post.id))
}

fn publish_default_post_for_trace(trace: &Trace, pool: &DbPool) -> Result<Option<Post>, PpdcError> {
    let Some(_) = ensure_default_draft_post_for_shared_trace(trace, pool)? else {
        return Ok(None);
    };

    let mut post = Post::find_for_trace(trace.id, pool)?.ok_or_else(|| {
        PpdcError::new(
            409,
            ErrorType::ApiError,
            "Trace post could not be found for this trace".to_string(),
        )
    })?;

    if post.status == crate::entities_v2::post::PostStatus::Archived {
        return Err(PpdcError::new(
            409,
            ErrorType::ApiError,
            "Trace post is archived and cannot be auto-published".to_string(),
        ));
    }

    if post.status == crate::entities_v2::post::PostStatus::Published {
        return Ok(Some(post));
    }

    post.status = crate::entities_v2::post::PostStatus::Published;
    if post.publishing_date.is_none() {
        post.publishing_date = Some(Utc::now().naive_utc());
    }
    let post = post.update(pool)?;
    JournalGrant::sync_effective_post_grants_for_post(&post, pool)?;

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
    if journal.status != JournalStatus::Active || journal.journal_type != JournalType::UserJournal {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Current persisted drafts are only available for active user journals".to_string(),
        ));
    }

    ensure_user_has_any_lens(user_id, pool)?;

    if let Some(existing_draft) = Trace::find_draft_for_journal(journal.id, &pool)? {
        let existing_draft =
            finalize_expired_trace_if_needed(existing_draft, pool, session_id).await?;
        if existing_draft.status == super::enums::TraceStatus::Draft {
            return apply_create_draft_payload(existing_draft, payload, user_id, session_id, pool);
        }
    }

    let draft = if let Some(draft) = Trace::find_draft_for_journal(journal.id, pool)? {
        draft
    } else {
        let draft = Trace::create_blank_draft_for_journal(&journal, pool)?;
        let mut journal = journal;
        journal.current_draft_id = Some(draft.id);
        let _ = journal.update(pool)?;
        draft
    };

    apply_create_draft_payload(draft, payload, user_id, session_id, pool)
}

fn apply_create_draft_payload(
    mut trace: Trace,
    payload: CreateJournalDraftDto,
    user_id: Uuid,
    session_id: Option<Uuid>,
    pool: &DbPool,
) -> Result<Trace, PpdcError> {
    let CreateJournalDraftDto {
        content,
        derived_from_trace_id,
        interaction_date,
        is_encrypted,
        encryption_metadata,
        content_image_asset_id,
        sharing_sensitivity,
        timeout_at,
    } = payload;
    let mut changed = false;

    if !content.is_empty() {
        trace.content = content;
        changed = true;
    }
    if derived_from_trace_id.is_some() {
        trace.derived_from_trace_id = derived_from_trace_id;
        changed = true;
    }
    if let Some(interaction_date) = interaction_date {
        trace.interaction_date = interaction_date;
        changed = true;
    }
    if let Some(is_encrypted) = is_encrypted {
        trace.is_encrypted = is_encrypted;
        changed = true;
    }
    if encryption_metadata.is_some() {
        trace.encryption_metadata = encryption_metadata;
        changed = true;
    }
    if content_image_asset_id.is_some() {
        trace.content_image_asset_id = content_image_asset_id;
        changed = true;
    }
    if let Some(sharing_sensitivity) = sharing_sensitivity {
        trace.sharing_sensitivity = sharing_sensitivity;
        changed = true;
    }
    if timeout_at.is_some() {
        trace.timeout_at = timeout_at;
        changed = true;
    }
    if trace.is_encrypted && trace.encryption_metadata.is_none() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "encryption_metadata is required when is_encrypted is true".to_string(),
        ));
    }
    if !trace.is_encrypted && trace.encryption_metadata.is_some() {
        trace.encryption_metadata = None;
        changed = true;
    }
    if let Some(timeout_at) = trace.timeout_at {
        validate_timeout_at(timeout_at)?;
    }
    if !changed {
        return Ok(trace);
    }
    let trace = trace.update(pool)?;
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

fn get_current_journal_draft(
    journal: &Journal,
    user_id: Uuid,
    pool: &DbPool,
) -> Result<Trace, PpdcError> {
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    if journal.status != JournalStatus::Active || journal.journal_type != JournalType::UserJournal {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Current persisted drafts are only available for active user journals".to_string(),
        ));
    }

    let draft_id = journal.current_draft_id.ok_or_else(|| {
        PpdcError::new(
            409,
            ErrorType::ApiError,
            "Journal current draft invariant is broken: current_draft_id is missing".to_string(),
        )
    })?;
    let trace = Trace::find_full_trace(draft_id, pool)?;
    if trace.user_id != user_id
        || trace.journal_id != Some(journal.id)
        || trace.status != TraceStatus::Draft
    {
        return Err(PpdcError::new(
            409,
            ErrorType::ApiError,
            "Journal current draft invariant is broken: current_draft_id does not point to a valid draft trace".to_string(),
        ));
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
    let trace =
        create_or_get_journal_draft(journal, user_id, Some(session.id), payload, &pool).await?;
    Ok(Json(trace))
}

#[debug_handler]
pub async fn post_finalized_journal_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    Json(payload): Json<CreateFinalizedJournalTraceDto>,
) -> Result<Json<CreateFinalizedJournalTraceResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    if journal.status != JournalStatus::Active || journal.journal_type != JournalType::UserJournal {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Finalized traces can only be created directly in active user journals".to_string(),
        ));
    }

    let mut seen_document_ids = HashSet::new();
    let mut attachment_documents = Vec::with_capacity(payload.document_attachments.len());
    for attachment in &payload.document_attachments {
        if !seen_document_ids.insert(attachment.document_id) {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "document_attachments cannot contain duplicate document_id values".to_string(),
            ));
        }
        let document = Document::find_full(attachment.document_id, &pool)?;
        if document.owner_user_id != user_id {
            return Err(PpdcError::unauthorized());
        }
        attachment_documents.push((attachment.clone(), document));
    }

    let has_meaningful_content = !payload.title.trim().is_empty()
        || !payload.content.trim().is_empty()
        || payload.content_image_asset_id.is_some()
        || !attachment_documents.is_empty();
    if !has_meaningful_content {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Cannot create an empty finalized trace".to_string(),
        ));
    }
    if payload.is_encrypted.unwrap_or(false) && payload.encryption_metadata.is_none() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "encryption_metadata is required when is_encrypted is true".to_string(),
        ));
    }

    let mut new_trace = NewTrace::new(
        payload.title,
        payload.subtitle,
        payload.content,
        payload
            .interaction_date
            .unwrap_or_else(|| Utc::now().naive_utc()),
        user_id,
        journal_id,
    );
    new_trace.derived_from_trace_id = payload.derived_from_trace_id;
    new_trace.is_encrypted = payload.is_encrypted.unwrap_or(false);
    new_trace.encryption_metadata = if new_trace.is_encrypted {
        payload.encryption_metadata
    } else {
        None
    };
    new_trace.content_image_asset_id = payload.content_image_asset_id;
    new_trace.sharing_sensitivity = payload
        .sharing_sensitivity
        .unwrap_or(TraceSharingSensitivity::Normal);
    new_trace.start_writing_at = new_trace.interaction_date;

    let trace = new_trace.create_finalized(&pool)?;
    let mut created_attachments = Vec::with_capacity(attachment_documents.len());
    for (attachment, document) in attachment_documents {
        let attachment_name = attachment
            .attachment_name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                let title = document.title.trim();
                if title.is_empty() {
                    "Attachment".to_string()
                } else {
                    title.to_string()
                }
            });

        NewTraceAttachment {
            trace_id: trace.id,
            document_id: document.id,
            attachment_name,
        }
        .create(&pool)?;
    }

    if !created_attachments.is_empty() || !seen_document_ids.is_empty() {
        created_attachments = TraceAttachment::find_with_documents_for_trace(trace.id, &pool)?;
    }

    let post = if payload.publish_default_post.unwrap_or(false) {
        publish_default_post_for_trace(&trace, &pool)?
    } else {
        None
    };
    let trace = Trace::find_full_trace(trace.id, &pool)?;

    Ok(Json(CreateFinalizedJournalTraceResponse {
        trace,
        attachments: created_attachments,
        post,
    }))
}

#[debug_handler]
pub async fn get_journal_draft_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
) -> Result<Json<Trace>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    let draft = get_current_journal_draft(&journal, user_id, &pool)?;
    Ok(Json(draft))
}

#[debug_handler]
pub async fn patch_journal_draft_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    Json(payload): Json<PatchTraceDto>,
) -> Result<Json<Trace>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let expected_version_integer =
        require_expected_version_integer(payload.expected_version_integer)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    let mut trace = get_current_journal_draft(&journal, user_id, &pool)?;

    if let Some(title) = payload.title {
        trace.title = title;
    }
    if let Some(content) = payload.content {
        trace.content = content;
    }
    if let Some(derived_from_trace_id) = payload.derived_from_trace_id {
        trace.derived_from_trace_id = derived_from_trace_id;
    }
    if let Some(interaction_date) = payload.interaction_date {
        trace.interaction_date = interaction_date;
    }
    if let Some(content_image_asset_id) = payload.content_image_asset_id {
        trace.content_image_asset_id = content_image_asset_id;
    }
    if let Some(sharing_sensitivity) = payload.sharing_sensitivity {
        trace.sharing_sensitivity = sharing_sensitivity;
    }
    if let Some(timeout_at) = payload.timeout_at {
        trace.timeout_at = timeout_at;
    }
    if let Some(timeout_at) = trace.timeout_at {
        validate_timeout_at(timeout_at)?;
    }

    let trace = trace.update_with_expected_version(expected_version_integer, &pool)?;
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
pub async fn put_trace_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTraceDto>,
) -> Result<Json<Trace>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let expected_version_integer =
        require_expected_version_integer(payload.expected_version_integer)?;
    let mut trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    trace = finalize_expired_trace_if_needed(trace, &pool, Some(session.id)).await?;

    let title_changed = payload
        .title
        .as_ref()
        .map(|title| title != &trace.title)
        .unwrap_or(false);
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
        .content_image_asset_id
        .map(|content_image_asset_id| content_image_asset_id != trace.content_image_asset_id)
        .unwrap_or(false);
    let sharing_sensitivity_changed = payload
        .sharing_sensitivity
        .map(|sharing_sensitivity| sharing_sensitivity != trace.sharing_sensitivity)
        .unwrap_or(false);
    let timeout_changed = payload
        .timeout_at
        .map(|timeout_at| timeout_at != trace.timeout_at)
        .unwrap_or(false);
    let publish_default_post = payload.publish_default_post.unwrap_or(false);

    match trace.status {
        super::enums::TraceStatus::Draft => {
            if let Some(title) = payload.title {
                trace.title = title;
            }
            if let Some(content) = payload.content {
                trace.content = content;
            }
            if let Some(derived_from_trace_id) = payload.derived_from_trace_id {
                trace.derived_from_trace_id = derived_from_trace_id;
            }
            if let Some(interaction_date) = payload.interaction_date {
                trace.interaction_date = interaction_date;
            }
            if let Some(content_image_asset_id) = payload.content_image_asset_id {
                trace.content_image_asset_id = content_image_asset_id;
            }
            if let Some(sharing_sensitivity) = payload.sharing_sensitivity {
                trace.sharing_sensitivity = sharing_sensitivity;
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
                        if trace.content.trim().is_empty()
                            && trace.content_image_asset_id.is_none()
                            && trace.is_blank
                        {
                            return Err(PpdcError::new(
                                400,
                                ErrorType::ApiError,
                                "Cannot finalize an empty trace".to_string(),
                            ));
                        }
                        let trace = finalize_trace_transition(
                            trace,
                            &pool,
                            Some(session.id),
                            false,
                            Some(expected_version_integer),
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
            if content_changed
                || title_changed
                || interaction_date_changed
                || sharing_sensitivity_changed
                || timeout_changed
            {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Cannot update title, content, interaction_date, sharing_sensitivity or timeout_at once trace is finalized".to_string(),
                ));
            }

            if let Some(content_image_asset_id) = payload.content_image_asset_id {
                trace.content_image_asset_id = content_image_asset_id;
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
            if content_changed
                || title_changed
                || interaction_date_changed
                || image_asset_changed
                || sharing_sensitivity_changed
                || timeout_changed
            {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Cannot update title, content, interaction_date, content_image_asset_id, sharing_sensitivity or timeout_at once trace is archived".to_string(),
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
                    trace.status = super::enums::TraceStatus::Finalized;
                    trace.timeout_at = None;
                }
            }
        }
    }

    let trace = trace.update_with_expected_version(expected_version_integer, &pool)?;
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
    let expected_version_integer =
        require_expected_version_integer(payload.expected_version_integer)?;
    let mut trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    trace = finalize_expired_trace_if_needed(trace, &pool, Some(session.id)).await?;
    if trace.status == super::enums::TraceStatus::Archived {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Only draft traces can be patched".to_string(),
        ));
    }

    if let Some(title) = payload.title {
        trace.title = title;
    }
    if let Some(content) = payload.content {
        trace.content = content;
    }
    if let Some(derived_from_trace_id) = payload.derived_from_trace_id {
        trace.derived_from_trace_id = derived_from_trace_id;
    }
    if let Some(interaction_date) = payload.interaction_date {
        trace.interaction_date = interaction_date;
    }
    if let Some(content_image_asset_id) = payload.content_image_asset_id {
        trace.content_image_asset_id = content_image_asset_id;
    }
    if let Some(sharing_sensitivity) = payload.sharing_sensitivity {
        trace.sharing_sensitivity = sharing_sensitivity;
    }
    if let Some(timeout_at) = payload.timeout_at {
        trace.timeout_at = timeout_at;
    }
    if let Some(timeout_at) = trace.timeout_at {
        validate_timeout_at(timeout_at)?;
    }

    let trace = trace.update_with_expected_version(expected_version_integer, &pool)?;
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
    Query(query): Query<TraceExpectedVersionQuery>,
    multipart: Multipart,
) -> Result<Json<TraceAssetUploadResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    trace = finalize_expired_trace_if_needed(trace, &pool, Some(session.id)).await?;
    if trace.status == super::enums::TraceStatus::Archived {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Archived traces cannot receive uploaded assets".to_string(),
        ));
    }

    let AssetUploadResponse {
        asset,
        signed_url,
        expires_at,
    } = upload_image_asset_for_user_from_multipart(user_id, &pool, multipart).await?;

    trace.content_image_asset_id = Some(asset.id);
    let trace = trace.update_with_expected_version(query.expected_version_integer, &pool)?;

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
) -> Result<Json<Vec<TraceAttachmentReadableView>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(id, &pool)?;
    if trace.user_id != user_id {
        let shared_post = find_published_trace_post(id, &pool)?;
        let can_read_shared_trace = match shared_post {
            Some(post) => PostGrant::user_can_read_post(&post, user_id, &pool)?,
            None => false,
        };
        if !can_read_shared_trace {
            return Err(PpdcError::unauthorized());
        }
    }

    let attachments = TraceAttachment::find_readable_for_trace(id, &pool)?;
    Ok(Json(attachments))
}

#[debug_handler]
pub async fn post_trace_document_attachment_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreateTraceDocumentAttachmentDto>,
) -> Result<Json<TraceAttachmentWithDocument>, PpdcError> {
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
            "Only draft traces can receive document attachments".to_string(),
        ));
    }

    let (document, attachment_name) = match payload {
        CreateTraceDocumentAttachmentDto::ExistingDocument {
            document_id,
            attachment_name,
        } => {
            let document = Document::find_full(document_id, &pool)?;
            if document.owner_user_id != user_id {
                return Err(PpdcError::unauthorized());
            }
            let attachment_name = attachment_name
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| document.title.trim().to_string());
            (document, attachment_name)
        }
        CreateTraceDocumentAttachmentDto::NewDocument {
            attachment_name,
            document,
        } => {
            let document = Document::create(document, user_id, &pool)?;
            let attachment_name = attachment_name
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| document.title.trim().to_string());
            (document, attachment_name)
        }
    };

    let attachment_name = if attachment_name.is_empty() {
        "Attachment".to_string()
    } else {
        attachment_name
    };

    let trace_id = trace.id;
    let attachment = NewTraceAttachment {
        trace_id: trace.id,
        document_id: document.id,
        attachment_name,
    }
    .create(&pool)?;
    let _ = trace.update(&pool)?;

    let attachment = TraceAttachment::find_with_documents_for_trace(trace_id, &pool)?
        .into_iter()
        .find(|candidate| candidate.id == attachment.id)
        .ok_or_else(|| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "Failed to hydrate trace document attachment".to_string(),
            )
        })?;

    Ok(Json(attachment))
}

#[debug_handler]
pub async fn put_trace_seen_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<TraceSeenStateResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let _trace = Trace::find_full_trace(id, &pool)?;
    let post = find_published_trace_post(id, &pool)?.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "Trace cannot be marked seen without a published post".to_string(),
        )
    })?;
    if !PostGrant::user_can_read_post(&post, user_id, &pool)? {
        return Err(PpdcError::unauthorized());
    }

    let state = UserPostState::create_or_mark_seen(user_id, post.id, &pool)?;
    let _ = create_usage_event(
        user_id,
        Some(session.id),
        UsageEventType::PostOpened,
        Some(post.id),
        None,
        &pool,
    );

    Ok(Json(TraceSeenStateResponse {
        trace_id: id,
        post_id: post.id,
        first_seen_at: state.first_seen_at,
        last_seen_at: state.last_seen_at,
    }))
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
    let document = Document::create(
        NewDocumentDto {
            status: None,
            document_role: DocumentRole::Reference,
            document_type: None,
            content_source: DocumentContentSource::InternalAsset,
            title: Some(attachment_name.clone()),
            subtitle: None,
            description: None,
            author_name: None,
            content: None,
            content_format: None,
            asset_id: Some(asset.id),
            external_content_url: None,
            cover_image_asset_id: None,
            cover_image_external_url: None,
        },
        user_id,
        &pool,
    )?;

    let trace_id = trace.id;
    let attachment = NewTraceAttachment {
        trace_id: trace.id,
        document_id: document.id,
        attachment_name,
    }
    .create(&pool)?;
    let _ = trace.update(&pool)?;
    let attachment = TraceAttachment::find_with_documents_for_trace(trace_id, &pool)?
        .into_iter()
        .find(|candidate| candidate.id == attachment.id)
        .ok_or_else(|| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "Failed to hydrate uploaded trace attachment".to_string(),
            )
        })?;

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
    let _ = trace.update(&pool)?;
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
    let expected_version_integer =
        require_expected_version_integer(payload.expected_version_integer)?;

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
    let trace = trace.update_with_expected_version(expected_version_integer, &pool)?;

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
) -> Result<Json<TraceReadableView>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(id, &pool)?;
    let is_owner = trace.user_id == session_user_id;
    if !is_owner && !trace.user_can_read(session_user_id, &pool)? {
        return Err(PpdcError::unauthorized());
    }
    let trace = finalize_expired_trace_if_needed(trace, &pool, Some(session.id)).await?;
    let view = trace_to_readable_view(trace, is_owner);
    let mut items = attach_seen_state_to_trace_readable_views(session_user_id, vec![view], &pool)?;
    Ok(Json(items.remove(0)))
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
        let shared_post = find_published_trace_post(trace_id, &pool)?;
        let can_read_shared_trace = match shared_post {
            Some(post) => PostGrant::user_can_read_post(&post, user_id, &pool)?,
            None => false,
        };
        if !can_read_shared_trace {
            return Err(PpdcError::unauthorized());
        }
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
    let sender_user = User::find(&user_id, &pool)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    let sender_is_owner = trace.user_id == user_id;
    let shared_post = find_published_trace_post(trace_id, &pool)?;
    if !sender_is_owner {
        let can_read_shared_trace = match shared_post.as_ref() {
            Some(post) => PostGrant::user_can_read_post(post, user_id, &pool)?,
            None => false,
        };
        if !can_read_shared_trace {
            return Err(PpdcError::unauthorized());
        }
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
        if !sender_user.allows_ai_features() {
            return Err(PpdcError::new(
                403,
                ErrorType::ApiError,
                "AI features are disabled for this account".to_string(),
            ));
        }
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
                "recipient_user_id must belong to a service mentor for mentor requests".to_string(),
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
            metadata: None,
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
            metadata: None,
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
        let Some(shared_post) = shared_post.as_ref() else {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Recipient must currently have access to a published shared trace post".to_string(),
            ));
        };
        if !PostGrant::user_can_read_post(shared_post, recipient_user_id, &pool)? {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Recipient must currently have access to the shared trace".to_string(),
            ));
        }
        recipient_user_id
    } else {
        if let Some(recipient_user_id) = payload.recipient_user_id {
            if recipient_user_id != trace.user_id {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Non-owners can only send trace messages to the trace owner".to_string(),
                ));
            }
        }
        trace.user_id
    };

    let message = NewMessage {
        sender_user_id: user_id,
        recipient_user_id,
        landscape_analysis_id: None,
        trace_id: Some(trace_id),
        post_id: shared_post.as_ref().map(|post| post.id),
        reply_to_message_id: None,
        message_type,
        processing_state: MessageProcessingState::Processed,
        title: payload.title.unwrap_or_default(),
        content: payload.content,
        attachment_type: payload.attachment_type,
        attachment: payload.attachment,
        metadata: None,
    }
    .create(&pool)?;
    notification::spawn_message_received_notification(message.clone(), pool.clone());

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
    Query(params): Query<JournalTracesQuery>,
) -> Result<Json<PaginatedResponse<JournalTraceView>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(id, &pool)?;
    let pagination = params.pagination.validate()?;
    let requested_status = params.status.unwrap_or(TraceStatus::Finalized);

    if requested_status == TraceStatus::Draft {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "GET /journals/:id/traces only supports status=finalized or status=archived"
                .to_string(),
        ));
    }

    if journal.user_id == user_id {
        let (traces, total) = Trace::get_for_journal_paginated(
            id,
            user_id,
            pagination.offset,
            pagination.limit,
            params.sharing_sensitivity,
            requested_status,
            params.seen,
            &pool,
        )?;
        let items = traces
            .into_iter()
            .map(|trace| JournalTraceView {
                id: trace.id,
                post_id: None,
                version_integer: Some(trace.version_integer),
                journal_id: id,
                title: trace.title,
                subtitle: Some(trace.subtitle),
                content: trace.content,
                derived_from_trace_id: trace.derived_from_trace_id,
                is_encrypted: Some(trace.is_encrypted),
                encryption_metadata: trace.encryption_metadata,
                content_image_asset_id: trace.content_image_asset_id,
                sharing_sensitivity: Some(trace.sharing_sensitivity),
                timeout_start_at: trace.timeout_start_at,
                timeout_at: trace.timeout_at,
                user_id: Some(trace.user_id),
                trace_type: Some(trace.trace_type),
                status: Some(trace.status),
                start_writing_at: Some(trace.start_writing_at),
                finalized_at: trace.finalized_at,
                seen: false,
                last_seen_at: None,
                interaction_date: trace.interaction_date,
                created_at: trace.created_at,
                updated_at: trace.updated_at,
            })
            .collect::<Vec<_>>();
        let items = attach_seen_state_to_journal_trace_views(user_id, items, &pool)?;
        return Ok(Json(PaginatedResponse::new(items, pagination, total)));
    }

    if requested_status == TraceStatus::Archived {
        return Err(PpdcError::unauthorized());
    }

    let (traces, total) = Trace::get_shared_for_journal_paginated(
        user_id,
        id,
        pagination.offset,
        pagination.limit,
        params.seen,
        &pool,
    )?;
    if total == 0 {
        return Err(PpdcError::unauthorized());
    }
    let traces = attach_seen_state_to_journal_trace_views(user_id, traces, &pool)?;
    Ok(Json(PaginatedResponse::new(traces, pagination, total)))
}
