use super::{model::*, notifications, service};
use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        message::{Message, NewMessageDto},
        session::Session,
    },
    pagination::{PaginatedResponse, PaginationParams},
};
use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, PpdcError> + Send + 'static,
) -> Result<T, PpdcError> {
    tokio::task::spawn_blocking(work).await.map_err(|_| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Conversation operation failed".into(),
        )
    })?
}
fn actor(s: &Session) -> Result<Uuid, PpdcError> {
    s.user_id.ok_or_else(PpdcError::unauthorized)
}
pub async fn create(
    Extension(pool): Extension<DbPool>,
    Extension(s): Extension<Session>,
    Json(input): Json<CreateConversation>,
) -> Result<(StatusCode, Json<ConversationView>), PpdcError> {
    let user = actor(&s)?;
    let status = if input.kind == ConversationKind::Direct {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let notification_pool = pool.clone();
    let created = blocking(move || service::create(user, input, &pool)).await?;
    notifications::spawn_members_added(created.added_members, notification_pool);
    Ok((status, Json(created.conversation)))
}
pub async fn list(
    Extension(pool): Extension<DbPool>,
    Extension(s): Extension<Session>,
    Query(q): Query<ConversationsQuery>,
) -> Result<Json<PaginatedResponse<ConversationView>>, PpdcError> {
    let user = actor(&s)?;
    blocking(move || {
        Ok(Json(service::list(
            user,
            q.pagination.validate()?,
            q.q.as_deref(),
            &pool,
        )?))
    })
    .await
}
pub async fn get(
    Extension(pool): Extension<DbPool>,
    Extension(s): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<ConversationView>, PpdcError> {
    let user = actor(&s)?;
    blocking(move || Ok(Json(service::get(user, id, &pool)?))).await
}
pub async fn patch(
    Extension(pool): Extension<DbPool>,
    Extension(s): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(input): Json<PatchConversation>,
) -> Result<Json<ConversationView>, PpdcError> {
    let user = actor(&s)?;
    blocking(move || Ok(Json(service::patch(user, id, input, &pool)?))).await
}
pub async fn members(
    Extension(pool): Extension<DbPool>,
    Extension(s): Extension<Session>,
    Path(id): Path<Uuid>,
    Query(q): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<MemberView>>, PpdcError> {
    let user = actor(&s)?;
    blocking(move || Ok(Json(service::members(user, id, q.validate()?, &pool)?))).await
}
pub async fn add_members(
    Extension(pool): Extension<DbPool>,
    Extension(s): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(input): Json<AddMembers>,
) -> Result<StatusCode, PpdcError> {
    let user = actor(&s)?;
    let notification_pool = pool.clone();
    let added = blocking(move || service::add_members(user, id, input.user_ids, &pool)).await?;
    notifications::spawn_members_added(added, notification_pool);
    Ok(StatusCode::NO_CONTENT)
}
pub async fn remove_member(
    Extension(pool): Extension<DbPool>,
    Extension(s): Extension<Session>,
    Path((id, target)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, PpdcError> {
    let user = actor(&s)?;
    blocking(move || {
        service::change_member(user, id, target, None, &pool)?;
        Ok(StatusCode::NO_CONTENT)
    })
    .await
}
pub async fn patch_member(
    Extension(pool): Extension<DbPool>,
    Extension(s): Extension<Session>,
    Path((id, target)): Path<(Uuid, Uuid)>,
    Json(input): Json<PatchMember>,
) -> Result<StatusCode, PpdcError> {
    let user = actor(&s)?;
    blocking(move || {
        service::change_member(user, id, target, Some(input.role), &pool)?;
        Ok(StatusCode::NO_CONTENT)
    })
    .await
}
pub async fn messages(
    Extension(pool): Extension<DbPool>,
    Extension(s): Extension<Session>,
    Path(id): Path<Uuid>,
    Query(q): Query<MessagesQuery>,
) -> Result<Json<PaginatedResponse<Message>>, PpdcError> {
    let user = actor(&s)?;
    blocking(move || {
        Ok(Json(service::messages(
            user,
            id,
            q.pagination.validate()?,
            q.before_position,
            &pool,
        )?))
    })
    .await
}
pub async fn read(
    Extension(pool): Extension<DbPool>,
    Extension(s): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(input): Json<ReadConversation>,
) -> Result<Json<MembershipView>, PpdcError> {
    let user = actor(&s)?;
    blocking(move || {
        Ok(Json(service::read(
            user,
            id,
            input.through_position,
            &pool,
        )?))
    })
    .await
}

#[derive(serde::Serialize)]
pub struct SendResponse {
    pub message: Message,
    pub pending_reply_message: Option<Message>,
}
pub async fn send(
    Extension(pool): Extension<DbPool>,
    Extension(s): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(input): Json<SendMessage>,
) -> Result<(StatusCode, Json<SendResponse>), PpdcError> {
    let user = actor(&s)?;
    let p = pool.clone();
    let conversation = blocking(move || service::get(user, id, &p)).await?;
    if conversation.kind == ConversationKind::Direct {
        let recipient = conversation
            .members_preview
            .iter()
            .find(|m| m.user.id != user)
            .map(|m| m.user.id)
            .ok_or_else(|| service::invalid("Cannot send to this direct conversation"))?;
        let response = crate::entities_v2::message::routes::post_message_route(
            Extension(pool),
            Extension(s),
            Json(NewMessageDto {
                recipient_user_id: recipient,
                landscape_analysis_id: input.landscape_analysis_id,
                trace_id: input.trace_id,
                post_id: input.post_id,
                reply_to_message_id: input.reply_to_message_id,
                message_type: input.message_type,
                title: input.title,
                content: input.content,
                attachment_type: input.attachment_type,
                attachment: input.attachment,
                writing_thread_id: input.writing_thread_id,
                expected_version_integer: input.expected_version_integer,
            }),
        )
        .await?
        .0;
        return Ok((
            StatusCode::CREATED,
            Json(SendResponse {
                message: response.question_message,
                pending_reply_message: response.pending_reply_message,
            }),
        ));
    }
    let p = pool.clone();
    let message = blocking(move || service::send(user, id, input, &p)).await?;
    crate::entities_v2::notification::spawn_message_received_notification(message.clone(), pool);
    Ok((
        StatusCode::CREATED,
        Json(SendResponse {
            message,
            pending_reply_message: None,
        }),
    ))
}
