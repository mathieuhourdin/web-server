use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query, RawQuery},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    journal::Journal,
    journal_grant::JournalGrant,
    platform_infra::mailer::{self, NewOutboundEmail, OutboundEmailProvider},
    post_grant::PostGrant,
    session::Session,
    trace::Trace,
    trace_attachment::TraceAttachment,
    user_post_state::{PostSeenByUser, UserPostState},
    user::{User, UserPrincipalType},
};
use crate::pagination::{PaginatedResponse, PaginationParams};
use chrono::Utc;
use serde::Deserialize;

use super::model::{
    legacy_lifecycle_for_status, NewPost, NewPostDto, Post, PostAudienceRole,
    PostInteractionType, PostStatus, PostType,
};

#[derive(Deserialize)]
pub struct PostFiltersQuery {
    pub is_external: Option<bool>,
    pub resource_type: Option<String>, // Legacy fallback mapped to post_type
    pub user_id: Option<Uuid>,
    pub journal_id: Option<Uuid>,
    pub status: Option<PostStatus>,
    pub seen: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct UserPostsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Deserialize)]
pub struct JournalPostsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Deserialize)]
pub struct TracePostsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Deserialize)]
pub struct DraftPostsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Deserialize)]
pub struct NewTracePostDto {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<String>,
    pub image_url: Option<String>,
    pub image_asset_id: Option<Uuid>,
    pub post_type: Option<PostType>,
    pub interaction_type: Option<PostInteractionType>,
    pub publishing_date: Option<chrono::NaiveDateTime>,
    pub status: Option<PostStatus>,
    pub audience_role: Option<PostAudienceRole>,
}

fn enqueue_post_published_notification_emails(
    post: &Post,
    pool: &DbPool,
) -> Result<Vec<Uuid>, PpdcError> {
    let Some(trace_id) = post.source_trace_id else {
        return Ok(vec![]);
    };

    let trace = Trace::find_full_trace(trace_id, pool)?;
    let Some(journal_id) = trace.journal_id else {
        return Ok(vec![]);
    };

    let journal = Journal::find_full(journal_id, pool)?;
    if journal.is_encrypted {
        return Ok(vec![]);
    }

    let owner = User::find(&post.user_id, pool)?;
    let recipient_ids = PostGrant::find_active_recipient_user_ids_for_post(post, pool)?;
    let recipients = User::find_many(&recipient_ids, pool)?;
    let journal_url = format!(
        "{}/me/journals/{}?post_id={}",
        crate::environment::get_app_base_url().trim_end_matches('/'),
        journal.id,
        post.id
    );
    let owner_display_name = owner.display_name();
    let scheduled_at = Some(Utc::now().naive_utc());
    let mut email_ids = Vec::new();

    for recipient in recipients.into_iter().filter(|recipient| {
        recipient.id != owner.id
            && recipient.principal_type == UserPrincipalType::Human
            && !recipient.email.trim().is_empty()
    }) {
        let template = mailer::shared_trace_finalized_email(
            &recipient.display_name(),
            &owner_display_name,
            &journal.title,
            &journal_url,
            trace.interaction_date,
            &post.content,
        );
        let email = NewOutboundEmail::new(
            Some(recipient.id),
            "POST_PUBLISHED".to_string(),
            Some("POST".to_string()),
            Some(post.id),
            recipient.email,
            "Matière Grise <noreply@ppdcoeur.fr>".to_string(),
            template.subject,
            template.text_body,
            template.html_body,
            OutboundEmailProvider::Resend,
            scheduled_at,
        )
        .create(pool)?;
        email_ids.push(email.id);
    }

    Ok(email_ids)
}

#[debug_handler]
pub async fn get_posts_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    RawQuery(raw_query): RawQuery,
    Query(filters): Query<PostFiltersQuery>,
) -> Result<Json<PaginatedResponse<Post>>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = PaginationParams::from_parts(Some(0), filters.limit).validate()?;
    let interaction_type =
        crate::pagination::parse_repeated_query_param(raw_query.as_deref(), "interaction_type")?;
    let post_type =
        crate::pagination::parse_repeated_query_param(raw_query.as_deref(), "post_type")?;
    let (posts, total) = Post::find_filtered_paginated(
        viewer_user_id,
        interaction_type,
        post_type,
        filters.resource_type,
        filters.is_external,
        filters.user_id,
        filters.journal_id,
        filters.status.or(Some(PostStatus::Published)),
        filters.seen,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(posts, pagination, total)))
}

#[debug_handler]
pub async fn get_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Post>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let post = Post::find_full(id, &pool)?;
    if post.user_id != viewer_user_id && post.status != PostStatus::Published {
        return Err(PpdcError::unauthorized());
    }
    if !PostGrant::user_can_read_post(&post, viewer_user_id, &pool)? {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(post))
}

#[debug_handler]
pub async fn get_post_attachments_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<crate::entities_v2::trace_attachment::TraceAttachmentWithAsset>>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let post = Post::find_full(id, &pool)?;
    let attachments = TraceAttachment::find_visible_for_post(&post, viewer_user_id, &pool)?;
    Ok(Json(attachments))
}

#[debug_handler]
pub async fn get_post_seen_by_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<PostSeenByUser>>, PpdcError> {
    let owner_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let seen_by = UserPostState::find_seen_by_for_post(owner_user_id, id, &pool)?;
    Ok(Json(seen_by))
}

#[debug_handler]
pub async fn get_user_posts_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<UserPostsQuery>,
) -> Result<Json<PaginatedResponse<Post>>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.pagination.validate()?;
    let interaction_type =
        crate::pagination::parse_repeated_query_param(raw_query.as_deref(), "interaction_type")?;
    let post_type =
        crate::pagination::parse_repeated_query_param(raw_query.as_deref(), "post_type")?;
    let (posts, total) = Post::find_for_user_filtered_paginated(
        viewer_user_id,
        user_id,
        interaction_type,
        post_type,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(posts, pagination, total)))
}

#[debug_handler]
pub async fn get_journal_posts_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    Query(params): Query<JournalPostsQuery>,
) -> Result<Json<PaginatedResponse<Post>>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    if !JournalGrant::user_can_read_journal(&journal, viewer_user_id, &pool)? {
        return Err(PpdcError::unauthorized());
    }
    let pagination = params.pagination.validate()?;
    let (posts, total) =
        Post::find_for_journal_paginated(viewer_user_id, journal_id, pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(posts, pagination, total)))
}

#[debug_handler]
pub async fn get_trace_posts_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
    Query(params): Query<TracePostsQuery>,
) -> Result<Json<PaginatedResponse<Post>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let pagination = params.pagination.validate()?;
    let (posts, total) =
        Post::find_for_trace_paginated(trace_id, pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(posts, pagination, total)))
}

#[debug_handler]
pub async fn get_post_drafts_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<DraftPostsQuery>,
) -> Result<Json<PaginatedResponse<Post>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.pagination.validate()?;
    let (posts, total) =
        Post::find_drafts_for_user_paginated(user_id, pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(posts, pagination, total)))
}

#[debug_handler]
pub async fn post_trace_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
    Json(payload): Json<NewTracePostDto>,
) -> Result<Json<Post>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let payload = NewPostDto {
        source_trace_id: Some(trace_id),
        title: payload.title.unwrap_or_else(|| trace.title.clone()),
        subtitle: payload.subtitle.or_else(|| Some(trace.subtitle.clone())),
        content: payload.content.unwrap_or_else(|| trace.content.clone()),
        image_url: payload.image_url,
        image_asset_id: payload.image_asset_id.or(trace.image_asset_id),
        post_type: payload.post_type,
        interaction_type: payload.interaction_type,
        publishing_date: payload.publishing_date,
        status: payload.status,
        audience_role: payload.audience_role,
    };

    let new_post = NewPost::new(payload, user_id);
    let post = new_post.create(&pool)?;
    if post.status == PostStatus::Published {
        match enqueue_post_published_notification_emails(&post, &pool) {
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
    }
    Ok(Json(post))
}

#[debug_handler]
pub async fn post_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewPostDto>,
) -> Result<Json<Post>, PpdcError> {
    let new_post = NewPost::new(payload, session.user_id.unwrap());
    let post = new_post.create(&pool)?;
    if post.status == PostStatus::Published {
        match enqueue_post_published_notification_emails(&post, &pool) {
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
    }
    Ok(Json(post))
}

#[debug_handler]
pub async fn put_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewPostDto>,
) -> Result<Json<Post>, PpdcError> {
    let mut post = Post::find_full(id, &pool)?;
    if post.user_id != session.user_id.unwrap() {
        return Err(PpdcError::unauthorized());
    }
    let previous_status = post.status;

    post.title = payload.title;
    post.source_trace_id = payload.source_trace_id;
    post.subtitle = payload.subtitle.unwrap_or_default();
    post.content = payload.content;
    post.image_url = payload.image_url;
    post.image_asset_id = payload.image_asset_id;
    post.publishing_date = payload.publishing_date;
    if let Some(interaction_type) = payload.interaction_type {
        post.interaction_type = interaction_type;
    }
    if let Some(post_type) = payload.post_type {
        post.post_type = post_type;
    }
    if let Some(status) = payload.status {
        post.status = status;
        let (publishing_state, maturing_state) = legacy_lifecycle_for_status(status);
        post.publishing_state = publishing_state;
        post.maturing_state = maturing_state;
    }
    if let Some(audience_role) = payload.audience_role {
        post.audience_role = audience_role;
    }
    if previous_status != PostStatus::Published
        && post.status == PostStatus::Published
        && post.publishing_date.is_none()
    {
        post.publishing_date = Some(Utc::now().naive_utc());
    }
    let post = post.update(&pool)?;

    if previous_status != PostStatus::Published && post.status == PostStatus::Published {
        match enqueue_post_published_notification_emails(&post, &pool) {
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
    }

    Ok(Json(post))
}
