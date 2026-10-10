use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::post_grants;

use super::enums::{PostGrantAccessLevel, PostGrantScope, PostGrantStatus};

#[derive(Serialize, Deserialize, Debug, Clone, QueryableByName)]
pub struct PostGrantSource {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub id: Uuid,
    #[serde(skip)]
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub post_grant_id: Uuid,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub source_type: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Uuid>)]
    pub audience_id: Option<Uuid>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub audience_name: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostGrant {
    pub id: Uuid,
    pub post_id: Uuid,
    pub owner_user_id: Uuid,
    pub grantee_user_id: Option<Uuid>,
    pub grantee_scope: Option<PostGrantScope>,
    pub audience_id: Option<Uuid>,
    pub sources: Vec<PostGrantSource>,
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
    Option<Uuid>,
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
        audience_id,
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
        audience_id,
        sources: Vec::new(),
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
    post_grants::audience_id,
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
        post_grants::audience_id,
    )
}

impl PostGrant {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<PostGrant, PpdcError> {
        let mut conn = pool.get()?;
        Self::find_with_conn(id, &mut conn)
    }

    pub(super) fn find_with_conn(
        id: Uuid,
        conn: &mut PgConnection,
    ) -> Result<PostGrant, PpdcError> {
        let row = post_grants::table
            .filter(post_grants::id.eq(id))
            .select(select_post_grant_columns())
            .first::<PostGrantTuple>(conn)
            .optional()?;
        let grant = row.map(tuple_to_post_grant).ok_or_else(|| {
            PpdcError::new(404, ErrorType::ApiError, "Post grant not found".to_string())
        })?;
        let mut grants = vec![grant];
        Self::hydrate_sources(&mut grants, conn)?;
        Ok(grants.remove(0))
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
        let mut grants = rows
            .into_iter()
            .map(tuple_to_post_grant)
            .collect::<Vec<_>>();
        Self::hydrate_sources(&mut grants, &mut conn)?;
        Ok((grants, total))
    }

    fn hydrate_sources(grants: &mut [PostGrant], conn: &mut PgConnection) -> Result<(), PpdcError> {
        if grants.is_empty() {
            return Ok(());
        }
        let ids = grants.iter().map(|g| g.id).collect::<Vec<_>>();
        let sources = diesel::sql_query(
            "SELECT s.id,s.post_grant_id,lower(s.source_type) AS source_type,
            s.audience_id,a.name AS audience_name,lower(s.status) AS status
            FROM post_grant_sources s LEFT JOIN audiences a ON a.id=s.audience_id
            WHERE s.post_grant_id=ANY($1) ORDER BY s.created_at,s.id",
        )
        .bind::<diesel::sql_types::Array<diesel::sql_types::Uuid>, _>(ids)
        .load::<PostGrantSource>(conn)?;
        let mut by_grant = std::collections::HashMap::<Uuid, Vec<PostGrantSource>>::new();
        for source in sources {
            by_grant
                .entry(source.post_grant_id)
                .or_default()
                .push(source);
        }
        for grant in grants {
            grant.sources = by_grant.remove(&grant.id).unwrap_or_default();
        }
        Ok(())
    }
}
