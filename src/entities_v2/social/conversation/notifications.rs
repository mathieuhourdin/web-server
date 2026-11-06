//! Best-effort membership pushes. Call only with events from a committed transaction.
use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        platform_infra::push,
        user::User,
    },
};
use chrono::NaiveDateTime;
use diesel::{
    prelude::*,
    sql_types::{Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(super) struct MemberAdded {
    pub conversation_id: Uuid,
    pub recipient_user_id: Uuid,
    pub actor_user_id: Uuid,
    pub joined_at: NaiveDateTime,
}

#[derive(QueryableByName)]
pub(super) struct GroupContext {
    #[diesel(sql_type = Nullable<Text>)]
    title: Option<String>,
}

pub(super) fn load_context(
    event: &MemberAdded,
    pool: &DbPool,
) -> Result<Option<GroupContext>, PpdcError> {
    // joined_at identifies the admission, so an old queued event cannot describe
    // a later rejoining. Blocks in groups don't revoke read access but do mute
    // actor-triggered pushes, just like messages and reactions.
    diesel::sql_query(
        "SELECT c.title FROM conversations c
        JOIN conversation_members cm ON cm.conversation_id=c.id
        WHERE c.id=$1 AND c.kind='GROUP' AND cm.user_id=$2 AND cm.joined_at=$3
          AND cm.left_at IS NULL AND cm.muted_at IS NULL AND cm.user_id<>$4
          AND conversation_readable(c.id,$2) AND conversation_readable(c.id,$4)
          AND NOT EXISTS(SELECT 1 FROM user_blocks b
            WHERE (b.blocker_user_id=$2 AND b.blocked_user_id=$4)
               OR (b.blocker_user_id=$4 AND b.blocked_user_id=$2))",
    )
    .bind::<SqlUuid, _>(event.conversation_id)
    .bind::<SqlUuid, _>(event.recipient_user_id)
    .bind::<Timestamp, _>(event.joined_at)
    .bind::<SqlUuid, _>(event.actor_user_id)
    .get_result(&mut *pool.get()?)
    .optional()
    .map_err(Into::into)
}

async fn context(event: MemberAdded, pool: DbPool) -> Result<Option<GroupContext>, PpdcError> {
    tokio::task::spawn_blocking(move || load_context(&event, &pool))
        .await
        .map_err(|_| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "Group notification worker failed".into(),
            )
        })?
}

async fn dispatch(event: &MemberAdded, pool: &DbPool) -> Result<(), PpdcError> {
    let Some(group) = context(event.clone(), pool.clone()).await? else {
        return Ok(());
    };
    let actor_id = event.actor_user_id;
    let actor_pool = pool.clone();
    let actor = tokio::task::spawn_blocking(move || User::find(&actor_id, &actor_pool))
        .await
        .map_err(|_| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                "Group notification actor lookup failed".into(),
            )
        })??;
    let notification = push::conversation_member_added_notification(
        event.conversation_id,
        &actor,
        group.title.as_deref(),
        event.joined_at,
        pool,
    )
    .await;
    // Avatar signing can take time; don't dispatch after removal, mute or block.
    if context(event.clone(), pool.clone()).await?.is_none() {
        return Ok(());
    }
    let result = push::send_to_user(event.recipient_user_id, notification, pool).await?;
    tracing::info!(target:"notification",conversation_id=%event.conversation_id,
        recipient_user_id=%event.recipient_user_id,actor_user_id=%event.actor_user_id,
        push_attempted_count=result.attempted_count,push_sent_count=result.sent_count,
        "conversation_member_added_push_completed");
    Ok(())
}

pub(super) fn spawn_members_added(events: Vec<MemberAdded>, pool: DbPool) {
    if events.is_empty() {
        return;
    }
    tokio::spawn(async move {
        for event in events {
            if let Err(error) = dispatch(&event, &pool).await {
                tracing::warn!(target:"notification",conversation_id=%event.conversation_id,
                    recipient_user_id=%event.recipient_user_id,actor_user_id=%event.actor_user_id,
                    error=%error.message,"conversation_member_added_push_failed");
            }
        }
    });
}
