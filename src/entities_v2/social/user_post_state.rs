use axum::{
    debug_handler,
    extract::{Extension, Json},
};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Array, BigInt, Nullable, Text, Timestamp, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    post::{Post, PostStatus},
    post_grant::PostGrant,
    session::Session,
    usage_event::{create_usage_event, UsageEventType},
    user::User,
};
use crate::schema::{posts, user_post_states};

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

#[derive(Serialize, Debug, Clone)]
pub struct PostSeenByUser {
    pub user_id: Uuid,
    pub display_name: String,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostSeenByPreviewUser {
    pub user_id: Uuid,
    pub display_name: String,
    pub profile_picture_asset_id: Option<Uuid>,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PostSeenByPreview {
    pub seen_count: i64,
    pub users: Vec<PostSeenByPreviewUser>,
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

#[derive(QueryableByName)]
struct PostSeenByPreviewRow {
    #[diesel(sql_type = SqlUuid)]
    trace_id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type = Text)]
    display_name: String,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    profile_picture_asset_id: Option<Uuid>,
    #[diesel(sql_type = Timestamp)]
    first_seen_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    last_seen_at: NaiveDateTime,
    #[diesel(sql_type = BigInt)]
    seen_count: i64,
}

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
        let mut conn = pool.get()?;
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

        let mut conn = pool.get()?;

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

    pub fn find_last_seen_at_by_user_and_trace_ids(
        user_id: Uuid,
        trace_ids: &[Uuid],
        pool: &DbPool,
    ) -> Result<HashMap<Uuid, NaiveDateTime>, PpdcError> {
        if trace_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = pool.get()?;
        let rows = posts::table
            .inner_join(user_post_states::table.on(user_post_states::post_id.eq(posts::id)))
            .filter(user_post_states::user_id.eq(user_id))
            .filter(posts::source_trace_id.eq_any(trace_ids))
            .select((posts::source_trace_id, user_post_states::last_seen_at))
            .load::<(Option<Uuid>, NaiveDateTime)>(&mut conn)?;

        let mut out = HashMap::with_capacity(rows.len());
        for (trace_id, last_seen_at) in rows {
            if let Some(trace_id) = trace_id {
                out.insert(trace_id, last_seen_at);
            }
        }
        Ok(out)
    }

    pub fn find_seen_trace_ids_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        let mut conn = pool.get()?;
        let rows = posts::table
            .inner_join(user_post_states::table.on(user_post_states::post_id.eq(posts::id)))
            .filter(user_post_states::user_id.eq(user_id))
            .select(posts::source_trace_id)
            .load::<Option<Uuid>>(&mut conn)?;

        Ok(rows.into_iter().flatten().collect())
    }

    pub fn find_seen_by_preview_by_trace_ids(
        owner_user_id: Uuid,
        trace_ids: &[Uuid],
        preview_limit: i64,
        pool: &DbPool,
    ) -> Result<HashMap<Uuid, PostSeenByPreview>, PpdcError> {
        if trace_ids.is_empty() || preview_limit <= 0 {
            return Ok(HashMap::new());
        }

        let mut conn = pool.get()?;
        let rows = sql_query(
            r#"
            WITH ranked_seen AS (
                SELECT
                    p.source_trace_id AS trace_id,
                    ups.user_id,
                    CASE
                        WHEN u.pseudonymized THEN u.pseudonym
                        ELSE TRIM(CONCAT(u.first_name, ' ', u.last_name))
                    END AS display_name,
                    u.profile_picture_asset_id,
                    ups.first_seen_at,
                    ups.last_seen_at,
                    COUNT(*) OVER (PARTITION BY ups.post_id) AS seen_count,
                    ROW_NUMBER() OVER (
                        PARTITION BY ups.post_id
                        ORDER BY ups.last_seen_at DESC
                    ) AS seen_rank
                FROM posts p
                INNER JOIN user_post_states ups ON ups.post_id = p.id
                INNER JOIN users u ON u.id = ups.user_id
                WHERE p.user_id = $1
                  AND p.source_trace_id = ANY($2)
                  AND ups.user_id <> $1
            )
            SELECT
                trace_id,
                user_id,
                display_name,
                profile_picture_asset_id,
                first_seen_at,
                last_seen_at,
                seen_count
            FROM ranked_seen
            WHERE seen_rank <= $3
            ORDER BY trace_id, last_seen_at DESC
            "#,
        )
        .bind::<SqlUuid, _>(owner_user_id)
        .bind::<Array<SqlUuid>, _>(trace_ids.to_vec())
        .bind::<BigInt, _>(preview_limit)
        .load::<PostSeenByPreviewRow>(&mut conn)?;

        let mut previews = HashMap::<Uuid, PostSeenByPreview>::new();
        for row in rows {
            let preview = previews.entry(row.trace_id).or_insert(PostSeenByPreview {
                seen_count: row.seen_count,
                users: Vec::new(),
            });
            preview.seen_count = row.seen_count;
            preview.users.push(PostSeenByPreviewUser {
                user_id: row.user_id,
                display_name: row.display_name,
                profile_picture_asset_id: row.profile_picture_asset_id,
                first_seen_at: row.first_seen_at,
                last_seen_at: row.last_seen_at,
            });
        }

        Ok(previews)
    }

    pub fn find_seen_by_for_post(
        owner_user_id: Uuid,
        post_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<PostSeenByUser>, PpdcError> {
        let post = Post::find_full(post_id, pool)?;
        if post.user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }

        let mut conn = pool.get()?;
        let rows = user_post_states::table
            .filter(user_post_states::post_id.eq(post_id))
            .order(user_post_states::last_seen_at.desc())
            .select((
                user_post_states::user_id,
                user_post_states::first_seen_at,
                user_post_states::last_seen_at,
            ))
            .load::<(Uuid, NaiveDateTime, NaiveDateTime)>(&mut conn)?;

        rows.into_iter()
            .map(|(user_id, first_seen_at, last_seen_at)| {
                let user = User::find(&user_id, pool)?;
                Ok(PostSeenByUser {
                    user_id,
                    display_name: user.display_name(),
                    first_seen_at,
                    last_seen_at,
                })
            })
            .collect()
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
    let _ = create_usage_event(
        user_id,
        Some(session.id),
        UsageEventType::PostOpened,
        Some(payload.post_id),
        None,
        &pool,
    );
    Ok(Json(state))
}
