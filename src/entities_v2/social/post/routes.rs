use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query, RawQuery},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    album::Album,
    document::{Document, DocumentStatus},
    error::{ErrorType, PpdcError},
    journal::Journal,
    journal_grant::JournalGrant,
    platform_infra::mailer::{self, NewOutboundEmail, OutboundEmail, OutboundEmailProvider},
    post_grant::PostGrant,
    session::Session,
    source_projection::SourceProjection,
    trace::Trace,
    trace_attachment::TraceAttachment,
    user::{User, UserPrincipalType},
    user_post_state::{PostSeenByUser, UserPostState},
};
use crate::pagination::{PaginatedResponse, PaginationParams};
use chrono::Utc;
use serde::Deserialize;

use super::model::{
    FeedPostResponse, NewPost, NewPostDto, Post, PostAudienceRole, PostInteractionType, PostStatus,
    PostType,
};
use super::persist::ensure_source_permits_published_post;

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
pub struct FeedPostsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Deserialize)]
pub struct JournalPostsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Deserialize)]
pub struct DraftPostsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Deserialize)]
pub struct PutTracePostDto {
    pub post_type: Option<PostType>,
    pub interaction_type: Option<PostInteractionType>,
    pub publishing_date: Option<Option<chrono::NaiveDateTime>>,
    pub status: Option<PostStatus>,
    pub audience_role: Option<PostAudienceRole>,
}

#[derive(Deserialize)]
pub struct PutSourceRecordPostDto {
    pub publishing_date: Option<Option<chrono::NaiveDateTime>>,
    pub status: Option<PostStatus>,
    pub audience_role: Option<PostAudienceRole>,
    pub post_type: Option<PostType>,
    pub interaction_type: Option<PostInteractionType>,
}

#[derive(Deserialize)]
pub struct UpdatePostDto {
    pub source_trace_id: Option<Option<Uuid>>,
    pub source_document_id: Option<Option<Uuid>>,
    pub source_album_id: Option<Option<Uuid>>,
    pub post_type: Option<PostType>,
    pub interaction_type: Option<PostInteractionType>,
    pub publishing_date: Option<Option<chrono::NaiveDateTime>>,
    pub status: Option<PostStatus>,
    pub audience_role: Option<PostAudienceRole>,
}

pub(crate) fn enqueue_post_published_notification_emails(
    post: &Post,
    pool: &DbPool,
) -> Result<Vec<Uuid>, PpdcError> {
    const POST_PUBLISHED_EMAIL_REASON: &str = "POST_PUBLISHED";
    const POST_PUBLISHED_INSTANT_EMAIL_MAX_PER_24H: i64 = 2;

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
            && recipient.allows_instant_shared_journal_activity_email()
    }) {
        let recent_sent_count = OutboundEmail::count_recent_sent_for_recipient_and_reason(
            recipient.id,
            POST_PUBLISHED_EMAIL_REASON,
            24,
            pool,
        )?;
        if recent_sent_count >= POST_PUBLISHED_INSTANT_EMAIL_MAX_PER_24H {
            continue;
        }

        let template = mailer::shared_trace_finalized_email(
            &recipient.display_name(),
            &owner_display_name,
            &journal.title,
            &journal_url,
            trace.interaction_date,
            &trace.content,
        );
        let email = NewOutboundEmail::new(
            Some(recipient.id),
            POST_PUBLISHED_EMAIL_REASON.to_string(),
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

fn dispatch_post_published_notification_emails(post: &Post, pool: &DbPool) {
    match enqueue_post_published_notification_emails(post, pool) {
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

fn apply_source_backed_projection(post: &mut Post, projection: &SourceProjection) {
    if matches!(post.post_type, PostType::Idea) {
        post.post_type = projection.default_post_type;
    }
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
) -> Result<Json<Vec<crate::entities_v2::trace_attachment::TraceAttachmentReadableView>>, PpdcError>
{
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
pub async fn get_feed_posts_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<FeedPostsQuery>,
) -> Result<Json<PaginatedResponse<FeedPostResponse>>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.pagination.validate()?;
    let (posts, total) =
        Post::find_feed_paginated(viewer_user_id, pagination.offset, pagination.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(posts, pagination, total)))
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
    let (posts, total) = Post::find_for_journal_paginated(
        viewer_user_id,
        journal_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(posts, pagination, total)))
}

#[debug_handler]
pub async fn get_trace_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
) -> Result<Json<Option<Post>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    Ok(Json(Post::find_for_trace(trace_id, &pool)?))
}

#[debug_handler]
pub async fn get_document_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(document_id): Path<Uuid>,
) -> Result<Json<Option<Post>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let document = Document::find_full(document_id, &pool)?;
    if document.owner_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    Ok(Json(Post::find_for_document(document_id, &pool)?))
}

#[debug_handler]
pub async fn get_album_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(album_id): Path<Uuid>,
) -> Result<Json<Option<Post>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let album = Album::find(album_id, &pool)?;
    if album.owner_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    Ok(Json(Post::find_for_album(album_id, &pool)?))
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
pub async fn put_trace_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
    Json(payload): Json<PutTracePostDto>,
) -> Result<Json<Post>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let mut post = if let Some(existing_post) = Post::find_for_trace(trace_id, &pool)? {
        existing_post
    } else {
        NewPost {
            source_trace_id: Some(trace_id),
            source_document_id: None,
            source_album_id: None,
            post_type: PostType::Idea,
            interaction_type: PostInteractionType::Output,
            user_id,
            publishing_date: None,
            status: PostStatus::Draft,
            audience_role: PostAudienceRole::Default,
        }
        .create(&pool)?
    };

    let previous_status = post.status;
    post.source_trace_id = Some(trace_id);
    post.source_document_id = None;
    post.source_album_id = None;

    if let Some(publishing_date) = payload.publishing_date {
        post.publishing_date = publishing_date;
    }
    if let Some(interaction_type) = payload.interaction_type {
        post.interaction_type = interaction_type;
    }
    if let Some(post_type) = payload.post_type {
        post.post_type = post_type;
    }
    if let Some(status) = payload.status {
        post.status = status;
    }
    if let Some(audience_role) = payload.audience_role {
        post.audience_role = audience_role;
    }
    if post.status == PostStatus::Published {
        ensure_source_permits_published_post(trace.status.permits_published_post())?;
    }
    if previous_status != PostStatus::Published
        && post.status == PostStatus::Published
        && post.publishing_date.is_none()
    {
        post.publishing_date = Some(Utc::now().naive_utc());
    }

    let post = post.update(&pool)?;
    if previous_status != PostStatus::Published && post.status == PostStatus::Published {
        JournalGrant::sync_effective_post_grants_for_post(&post, &pool)?;
        dispatch_post_published_notification_emails(&post, &pool);
    }
    Ok(Json(post))
}

#[debug_handler]
pub async fn post_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewPostDto>,
) -> Result<Json<Post>, PpdcError> {
    if payload.source_trace_id.is_some()
        || payload.source_document_id.is_some()
        || payload.source_album_id.is_some()
    {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "POST /posts only supports custom posts without trace, document or album linkage"
                .to_string(),
        ));
    }
    let new_post = NewPost::new(payload, session.user_id.unwrap());
    let post = new_post.create(&pool)?;
    if post.status == PostStatus::Published {
        dispatch_post_published_notification_emails(&post, &pool);
    }
    Ok(Json(post))
}

#[debug_handler]
pub async fn put_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdatePostDto>,
) -> Result<Json<Post>, PpdcError> {
    let mut post = Post::find_full(id, &pool)?;
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if post.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    if payload.source_trace_id.is_some()
        || payload.source_document_id.is_some()
        || payload.source_album_id.is_some()
    {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Use dedicated source routes to manage trace, document or album linkage".to_string(),
        ));
    }
    if post.source_trace_id.is_some()
        || post.source_document_id.is_some()
        || post.source_album_id.is_some()
    {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Use dedicated source routes to update trace-backed, document-backed or album-backed posts"
                .to_string(),
        ));
    }
    let previous_status = post.status;
    if let Some(publishing_date) = payload.publishing_date {
        post.publishing_date = publishing_date;
    }
    if let Some(interaction_type) = payload.interaction_type {
        post.interaction_type = interaction_type;
    }
    if let Some(post_type) = payload.post_type {
        post.post_type = post_type;
    }
    if let Some(status) = payload.status {
        post.status = status;
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
        JournalGrant::sync_effective_post_grants_for_post(&post, &pool)?;
        dispatch_post_published_notification_emails(&post, &pool);
    }

    Ok(Json(post))
}

#[debug_handler]
pub async fn put_document_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(document_id): Path<Uuid>,
    Json(payload): Json<PutSourceRecordPostDto>,
) -> Result<Json<Post>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let document = Document::find_full(document_id, &pool)?;
    if document.owner_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    if document.status == DocumentStatus::Archived {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Archived documents cannot be published".to_string(),
        ));
    }

    let mut post = if let Some(existing_post) = Post::find_for_document(document_id, &pool)? {
        existing_post
    } else {
        let projection = SourceProjection::from_document(&document);
        NewPost {
            source_trace_id: None,
            source_document_id: Some(document_id),
            source_album_id: None,
            post_type: projection.default_post_type,
            interaction_type: PostInteractionType::Output,
            user_id,
            publishing_date: None,
            status: PostStatus::Draft,
            audience_role: PostAudienceRole::Default,
        }
        .create(&pool)?
    };
    if post.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let previous_status = post.status;
    post.source_trace_id = None;
    post.source_document_id = Some(document_id);
    post.source_album_id = None;
    let projection = SourceProjection::from_document(&document);
    apply_source_backed_projection(&mut post, &projection);

    if let Some(publishing_date) = payload.publishing_date {
        post.publishing_date = publishing_date;
    }
    if let Some(interaction_type) = payload.interaction_type {
        post.interaction_type = interaction_type;
    }
    if let Some(post_type) = payload.post_type {
        post.post_type = post_type;
    }
    if let Some(status) = payload.status {
        post.status = status;
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
        JournalGrant::sync_effective_post_grants_for_post(&post, &pool)?;
        dispatch_post_published_notification_emails(&post, &pool);
    }
    Ok(Json(post))
}

#[debug_handler]
pub async fn put_album_post_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(album_id): Path<Uuid>,
    Json(payload): Json<PutSourceRecordPostDto>,
) -> Result<Json<Post>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let album = Album::find(album_id, &pool)?;
    if album.owner_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let mut post = if let Some(existing_post) = Post::find_for_album(album_id, &pool)? {
        existing_post
    } else {
        let projection = SourceProjection::from_album(&album);
        NewPost {
            source_trace_id: None,
            source_document_id: None,
            source_album_id: Some(album_id),
            post_type: projection.default_post_type,
            interaction_type: PostInteractionType::Output,
            user_id,
            publishing_date: None,
            status: PostStatus::Draft,
            audience_role: PostAudienceRole::Default,
        }
        .create(&pool)?
    };
    if post.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let previous_status = post.status;
    post.source_trace_id = None;
    post.source_document_id = None;
    post.source_album_id = Some(album_id);
    let projection = SourceProjection::from_album(&album);
    apply_source_backed_projection(&mut post, &projection);

    if let Some(publishing_date) = payload.publishing_date {
        post.publishing_date = publishing_date;
    }
    if let Some(interaction_type) = payload.interaction_type {
        post.interaction_type = interaction_type;
    }
    if let Some(post_type) = payload.post_type {
        post.post_type = post_type;
    }
    if let Some(status) = payload.status {
        post.status = status;
    }
    if let Some(audience_role) = payload.audience_role {
        post.audience_role = audience_role;
    }
    if post.status == PostStatus::Published {
        ensure_source_permits_published_post(album.completion_status.permits_published_post())?;
    }
    if previous_status != PostStatus::Published
        && post.status == PostStatus::Published
        && post.publishing_date.is_none()
    {
        post.publishing_date = Some(Utc::now().naive_utc());
    }

    let post = post.update(&pool)?;
    if previous_status != PostStatus::Published && post.status == PostStatus::Published {
        JournalGrant::sync_effective_post_grants_for_post(&post, &pool)?;
        dispatch_post_published_notification_emails(&post, &pool);
    }
    Ok(Json(post))
}
