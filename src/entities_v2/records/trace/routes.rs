use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use chrono::Utc;
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
        routes::enqueue_received_message_notification_email, Message, MessageAttachment,
        MessageAttachmentType, MessageProcessingState, MessageType, NewMessage,
    },
    post::{NewPost, Post, PostAudienceRole, PostInteractionType, PostType},
    post_grant::{NewPostGrantDto, PostGrantScope},
    platform_infra::mailer,
    session::Session,
    shared::MaturingState,
    user::User,
};
use crate::pagination::{PaginatedResponse, PaginationParams};
use crate::work_analyzer;

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
}

fn spawn_autoplay_lens_runs_for_trace(
    user_id: Uuid,
    trace_id: Uuid,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let user_lenses = Lens::get_user_lenses(user_id, pool)?;
    for lens in user_lenses.into_iter().filter(|lens| lens.autoplay) {
        let lens = if lens.target_trace_id != Some(trace_id) {
            lens.update_target_trace(Some(trace_id), pool)?
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

fn create_or_get_journal_draft(
    journal: Journal,
    user_id: Uuid,
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
        return Ok(existing_draft);
    }

    let CreateJournalDraftDto {
        content,
        interaction_date,
        is_encrypted,
        encryption_metadata,
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
    trace.create(pool)
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
    let trace = create_or_get_journal_draft(journal, user_id, payload, &pool)?;
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

    let draft = Trace::find_draft_for_journal(journal_id, &pool)?;
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

    let content_changed = payload
        .content
        .as_ref()
        .map(|content| content != &trace.content)
        .unwrap_or(false);
    let interaction_date_changed = payload
        .interaction_date
        .map(|interaction_date| interaction_date != trace.interaction_date)
        .unwrap_or(false);
    let has_explicit_interaction_date = payload.interaction_date.is_some();
    let previous_status = trace.status;

    match trace.status {
        super::enums::TraceStatus::Draft => {
            if let Some(content) = payload.content {
                trace.content = content;
            }
            if let Some(interaction_date) = payload.interaction_date {
                trace.interaction_date = interaction_date;
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
                        if !trace.is_encrypted {
                            let qualified =
                                llm_qualify::qualify_trace(trace.content.as_str()).await?;
                            trace.title = qualified.title;
                            trace.subtitle = qualified.subtitle;
                            if !has_explicit_interaction_date {
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
                    }
                    super::enums::TraceStatus::Archived => {
                        trace.status = super::enums::TraceStatus::Archived;
                    }
                }
            }
        }
        super::enums::TraceStatus::Finalized => {
            if content_changed || interaction_date_changed {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Cannot update content or interaction_date once trace is finalized".to_string(),
                ));
            }

            match payload.status {
                Some(super::enums::TraceStatus::Archived) => {
                    trace.status = super::enums::TraceStatus::Archived;
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
            if content_changed || interaction_date_changed {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Cannot update content or interaction_date once trace is archived".to_string(),
                ));
            }

            match payload.status {
                Some(super::enums::TraceStatus::Archived) | None => {}
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

    if previous_status == super::enums::TraceStatus::Draft
        && trace.status == super::enums::TraceStatus::Finalized
        && !trace.is_encrypted
    {
        enqueue_autoplay_lens_runs_for_trace(user_id, trace.id, pool.clone());
        if trace.trace_type == super::enums::TraceType::UserTrace {
            if let Err(err) = ensure_default_draft_post_for_shared_trace(&trace, &pool) {
                tracing::warn!(
                    target: "post",
                    "shared_trace_default_post_create_failed trace_id={} message={}",
                    trace.id,
                    err.message
                );
            }
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

    let trace = trace.update(&pool)?;
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
    let pagination = params.validate()?;
    let (traces, total) =
        Trace::get_all_for_user_paginated(user_id, pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(traces, pagination, total)))
}

#[debug_handler]
pub async fn get_drafts_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Trace>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
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
    let owner = if sender_is_owner {
        Some(User::find(&user_id, &pool)?)
    } else {
        None
    };
    let mentor_id = owner.as_ref().and_then(|user| user.mentor_id);
    let message_type = payload.message_type.unwrap_or_else(|| {
        if sender_is_owner {
            match (payload.recipient_user_id, mentor_id) {
                (None, _) => MessageType::Question,
                (Some(recipient_user_id), Some(mentor_id)) if recipient_user_id == mentor_id => {
                    MessageType::Question
                }
                _ => MessageType::General,
            }
        } else {
            MessageType::General
        }
    });

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

        let mentor_id = mentor_id.ok_or_else(|| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                "No mentor assigned to user".to_string(),
            )
        })?;

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
            recipient_user_id: mentor_id,
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
            sender_user_id: mentor_id,
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
