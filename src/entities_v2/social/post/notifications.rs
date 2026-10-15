//! Notification targeting is not an ACL and must never create grants.
use super::model::Post;
use crate::{
    db::DbPool,
    entities_v2::error::{ErrorType, PpdcError},
};
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationNotificationMode {
    #[default]
    All,
    None,
    Selected,
}
impl PublicationNotificationMode {
    pub fn to_db(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::None => "NONE",
            Self::Selected => "SELECTED",
        }
    }
    pub fn from_db(value: &str) -> Self {
        match value {
            "ALL" => Self::All,
            "SELECTED" => Self::Selected,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NotificationOptions {
    pub notification_mode: Option<PublicationNotificationMode>,
    pub notification_user_ids: Option<Vec<Uuid>>,
}

impl Post {
    pub fn notification_allows(&self, user: Uuid) -> bool {
        match self.notification_mode {
            PublicationNotificationMode::All => true,
            PublicationNotificationMode::None => false,
            PublicationNotificationMode::Selected => self.notification_user_ids.contains(&user),
        }
    }

    pub fn apply_notification_options(
        &mut self,
        options: NotificationOptions,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        if options.notification_mode.is_none() && options.notification_user_ids.is_none() {
            return Ok(());
        }
        let mode = options.notification_mode.unwrap_or(self.notification_mode);
        if mode != PublicationNotificationMode::Selected
            && options
                .notification_user_ids
                .as_ref()
                .is_some_and(|ids| !ids.is_empty())
        {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "notification_user_ids requires notification_mode=selected".to_string(),
            ));
        }
        let mut ids = options
            .notification_user_ids
            .unwrap_or_else(|| self.notification_user_ids.clone());
        if mode != PublicationNotificationMode::Selected {
            ids.clear();
        }
        ids.sort_unstable();
        ids.dedup();
        if mode == PublicationNotificationMode::Selected {
            if ids.is_empty() || ids.len() > 500 {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "selected notifications require between 1 and 500 notification_user_ids"
                        .to_string(),
                ));
            }
            // Validate prepared posts too: the ordinary recipient helper requires
            // an already-published post. Access is rechecked by dispatch as usual.
            #[derive(QueryableByName)]
            struct Count {
                #[diesel(sql_type=BigInt)]
                count: i64,
            }
            let eligible = diesel::sql_query("SELECT COUNT(*)::bigint AS count FROM users u
                JOIN posts p ON p.id=$1
                LEFT JOIN traces t ON t.id=p.source_trace_id
                LEFT JOIN traces parent ON parent.id=t.parent_trace_id
                WHERE u.id=ANY($2) AND u.id<>p.user_id AND automation_same_sandbox(p.user_id,u.id)
                AND EXISTS(SELECT 1 FROM post_grants g WHERE g.post_id=p.id AND g.status='ACTIVE'
                    AND (g.grantee_user_id=u.id OR (g.grantee_scope='ALL_PLATFORM_USERS'
                        AND u.is_platform_user AND u.principal_type='HUMAN')))
                AND NOT EXISTS(SELECT 1 FROM user_blocks b
                    WHERE (b.blocker_user_id=u.id AND b.blocked_user_id IN(p.user_id,t.user_id,parent.user_id))
                        OR (b.blocked_user_id=u.id AND b.blocker_user_id IN(p.user_id,t.user_id,parent.user_id)))")
                .bind::<SqlUuid,_>(self.id).bind::<Array<SqlUuid>,_>(&ids)
                .get_result::<Count>(&mut pool.get()?)?.count;
            if eligible != ids.len() as i64 {
                return Err(PpdcError::new(400,ErrorType::ApiError,
                    "Every notification recipient must already be an eligible active grantee of this post".to_string()));
            }
        }
        self.notification_mode = mode;
        self.notification_user_ids = ids;
        Ok(())
    }
}

#[derive(Serialize)]
pub struct NotificationSettings {
    pub notification_mode: PublicationNotificationMode,
    pub notification_user_ids: Vec<Uuid>,
}

pub async fn get_notification_settings(
    axum::extract::Extension(pool): axum::extract::Extension<DbPool>,
    axum::extract::Extension(session): axum::extract::Extension<
        crate::entities_v2::session::Session,
    >,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<axum::Json<NotificationSettings>, PpdcError> {
    let owner = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let post = Post::find_full(id, &pool)?;
    if post.user_id != owner {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "Post not found".to_string(),
        ));
    }
    Ok(axum::Json(NotificationSettings {
        notification_mode: post.notification_mode,
        notification_user_ids: post.notification_user_ids,
    }))
}
