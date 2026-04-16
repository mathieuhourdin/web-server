use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::post_grants;

use super::enums::{PostGrantAccessLevel, PostGrantScope, PostGrantStatus};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostGrant {
    pub id: Uuid,
    pub post_id: Uuid,
    pub owner_user_id: Uuid,
    pub grantee_user_id: Option<Uuid>,
    pub grantee_scope: Option<PostGrantScope>,
    pub access_level: PostGrantAccessLevel,
    pub status: PostGrantStatus,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NewPostGrantDto {
    pub grantee_user_id: Option<Uuid>,
    pub grantee_scope: Option<PostGrantScope>,
    pub access_level: Option<PostGrantAccessLevel>,
}

type PostGrantTuple = (
    Uuid,
    Uuid,
    Uuid,
    Option<Uuid>,
    Option<String>,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

fn tuple_to_post_grant(row: PostGrantTuple) -> PostGrant {
    let (
        id,
        post_id,
        owner_user_id,
        grantee_user_id,
        grantee_scope_raw,
        access_level_raw,
        status_raw,
        created_at,
        updated_at,
    ) = row;

    PostGrant {
        id,
        post_id,
        owner_user_id,
        grantee_user_id,
        grantee_scope: grantee_scope_raw
            .as_deref()
            .and_then(PostGrantScope::from_db),
        access_level: PostGrantAccessLevel::from_db(&access_level_raw),
        status: PostGrantStatus::from_db(&status_raw),
        created_at,
        updated_at,
    }
}

fn select_post_grant_columns() -> (
    post_grants::id,
    post_grants::post_id,
    post_grants::owner_user_id,
    post_grants::grantee_user_id,
    post_grants::grantee_scope,
    post_grants::access_level,
    post_grants::status,
    post_grants::created_at,
    post_grants::updated_at,
) {
    (
        post_grants::id,
        post_grants::post_id,
        post_grants::owner_user_id,
        post_grants::grantee_user_id,
        post_grants::grantee_scope,
        post_grants::access_level,
        post_grants::status,
        post_grants::created_at,
        post_grants::updated_at,
    )
}

impl PostGrant {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<PostGrant, PpdcError> {
        let mut conn = pool.get()?;
        let row = post_grants::table
            .filter(post_grants::id.eq(id))
            .select(select_post_grant_columns())
            .first::<PostGrantTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_post_grant).ok_or_else(|| {
            PpdcError::new(404, ErrorType::ApiError, "Post grant not found".to_string())
        })
    }

    pub fn find_for_post_paginated(
        post_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<PostGrant>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let total = post_grants::table
            .filter(post_grants::post_id.eq(post_id))
            .count()
            .get_result::<i64>(&mut conn)?;
        let rows = post_grants::table
            .filter(post_grants::post_id.eq(post_id))
            .select(select_post_grant_columns())
            .order(post_grants::updated_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<PostGrantTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_post_grant).collect(), total))
    }
}
