use axum::{
    debug_handler,
    extract::{Extension, Json},
};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Uuid as SqlUuid;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    post::{Post, PostStatus},
    post_grant::PostGrant,
    session::Session,
};
use crate::schema::user_post_states;

#[derive(Serialize, Debug, Clone)]
pub struct UserPostState {
    pub id: Uuid,
    pub user_id: Uuid,
    pub post_id: Uuid,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NewUserPostStateDto {
    pub post_id: Uuid,
}

type UserPostStateTuple = (
    Uuid,
    Uuid,
    Uuid,
    NaiveDateTime,
    NaiveDateTime,
    NaiveDateTime,
    NaiveDateTime,
);

impl From<UserPostStateTuple> for UserPostState {
    fn from(row: UserPostStateTuple) -> Self {
        let (id, user_id, post_id, first_seen_at, last_seen_at, created_at, updated_at) = row;
        Self {
            id,
            user_id,
            post_id,
            first_seen_at,
            last_seen_at,
            created_at,
            updated_at,
        }
    }
}

impl UserPostState {
    fn find_by_user_and_post(
        user_id: Uuid,
        post_id: Uuid,
        pool: &DbPool,
    ) -> Result<UserPostState, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let row = user_post_states::table
            .filter(user_post_states::user_id.eq(user_id))
            .filter(user_post_states::post_id.eq(post_id))
            .select((
                user_post_states::id,
                user_post_states::user_id,
                user_post_states::post_id,
                user_post_states::first_seen_at,
                user_post_states::last_seen_at,
                user_post_states::created_at,
                user_post_states::updated_at,
            ))
            .first::<UserPostStateTuple>(&mut conn)?;
        Ok(row.into())
    }

    pub fn create_or_mark_seen(
        user_id: Uuid,
        post_id: Uuid,
        pool: &DbPool,
    ) -> Result<UserPostState, PpdcError> {
        let post = Post::find_full(post_id, pool)?;
        if post.user_id != user_id && post.status != PostStatus::Published {
            return Err(PpdcError::unauthorized());
        }
        if !PostGrant::user_can_read_post(&post, user_id, pool)? {
            return Err(PpdcError::unauthorized());
        }

        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        sql_query(
            r#"
            INSERT INTO user_post_states (
                user_id,
                post_id,
                first_seen_at,
                last_seen_at,
                created_at,
                updated_at
            )
            VALUES ($1, $2, NOW(), NOW(), NOW(), NOW())
            ON CONFLICT (user_id, post_id)
            DO UPDATE SET
                last_seen_at = NOW(),
                updated_at = NOW()
            "#,
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<SqlUuid, _>(post_id)
        .execute(&mut conn)?;

        Self::find_by_user_and_post(user_id, post_id, pool)
    }
}

#[debug_handler]
pub async fn post_user_post_state_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewUserPostStateDto>,
) -> Result<Json<UserPostState>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let state = UserPostState::create_or_mark_seen(user_id, payload.post_id, &pool)?;
    Ok(Json(state))
}
