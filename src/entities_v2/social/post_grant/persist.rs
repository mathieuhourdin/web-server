use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::post::Post;
use crate::entities_v2::relationship::Relationship;
use crate::schema::post_grants;

use super::enums::{PostGrantAccessLevel, PostGrantScope, PostGrantStatus};
use super::model::{NewPostGrantDto, PostGrant};

impl PostGrant {
    fn validate_target_fields(
        payload: &NewPostGrantDto,
    ) -> Result<(Option<Uuid>, Option<PostGrantScope>, PostGrantAccessLevel), PpdcError> {
        match (payload.grantee_user_id, payload.grantee_scope) {
            (Some(grantee_user_id), None) => Ok((
                Some(grantee_user_id),
                None,
                payload.access_level.unwrap_or(PostGrantAccessLevel::Read),
            )),
            (None, Some(scope)) => Ok((
                None,
                Some(scope),
                payload.access_level.unwrap_or(PostGrantAccessLevel::Read),
            )),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Provide either grantee_user_id or grantee_scope".to_string(),
            )),
        }
    }

    fn find_existing(
        post_id: Uuid,
        grantee_user_id: Option<Uuid>,
        grantee_scope: Option<PostGrantScope>,
        pool: &DbPool,
    ) -> Result<Option<PostGrant>, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let mut query = post_grants::table
            .filter(post_grants::post_id.eq(post_id))
            .into_boxed();

        if let Some(grantee_user_id) = grantee_user_id {
            query = query.filter(post_grants::grantee_user_id.eq(Some(grantee_user_id)));
        } else if let Some(scope) = grantee_scope {
            query = query.filter(post_grants::grantee_scope.eq(Some(scope.to_db())));
        }

        let row = query
            .select((
                post_grants::id,
                post_grants::post_id,
                post_grants::owner_user_id,
                post_grants::grantee_user_id,
                post_grants::grantee_scope,
                post_grants::access_level,
                post_grants::status,
                post_grants::created_at,
                post_grants::updated_at,
            ))
            .first::<(
                Uuid,
                Uuid,
                Uuid,
                Option<Uuid>,
                Option<String>,
                String,
                String,
                chrono::NaiveDateTime,
                chrono::NaiveDateTime,
            )>(&mut conn)
            .optional()?;

        row.map(|row| {
            Ok(PostGrant {
                id: row.0,
                post_id: row.1,
                owner_user_id: row.2,
                grantee_user_id: row.3,
                grantee_scope: row.4.as_deref().and_then(PostGrantScope::from_db),
                access_level: PostGrantAccessLevel::from_db(&row.5),
                status: PostGrantStatus::from_db(&row.6),
                created_at: row.7,
                updated_at: row.8,
            })
        })
        .transpose()
    }

    pub fn create_or_update(
        post: &Post,
        owner_user_id: Uuid,
        payload: NewPostGrantDto,
        pool: &DbPool,
    ) -> Result<PostGrant, PpdcError> {
        if post.user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }

        let (grantee_user_id, grantee_scope, access_level) =
            Self::validate_target_fields(&payload)?;

        if let Some(grantee_user_id) = grantee_user_id {
            if grantee_user_id == owner_user_id {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Cannot grant a post to yourself".to_string(),
                ));
            }
            let accepted = Relationship::is_follow_accepted(grantee_user_id, owner_user_id, pool)?;
            if !accepted {
                return Err(PpdcError::new(
                    403,
                    ErrorType::ApiError,
                    "Direct post grants are restricted to accepted followers".to_string(),
                ));
            }
        }

        if let Some(existing) = Self::find_existing(post.id, grantee_user_id, grantee_scope, pool)?
        {
            let mut conn = pool.get().expect("Failed to get a connection from the pool");
            diesel::update(post_grants::table.filter(post_grants::id.eq(existing.id)))
                .set((
                    post_grants::access_level.eq(access_level.to_db()),
                    post_grants::status.eq(PostGrantStatus::Active.to_db()),
                    post_grants::updated_at.eq(diesel::dsl::now),
                ))
                .execute(&mut conn)?;
            return PostGrant::find(existing.id, pool);
        }

        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let id = Uuid::new_v4();
        diesel::insert_into(post_grants::table)
            .values((
                post_grants::id.eq(id),
                post_grants::post_id.eq(post.id),
                post_grants::owner_user_id.eq(owner_user_id),
                post_grants::grantee_user_id.eq(grantee_user_id),
                post_grants::grantee_scope.eq(grantee_scope.map(|scope| scope.to_db())),
                post_grants::access_level.eq(access_level.to_db()),
                post_grants::status.eq(PostGrantStatus::Active.to_db()),
            ))
            .execute(&mut conn)?;
        PostGrant::find(id, pool)
    }

    pub fn revoke(
        post_id: Uuid,
        grant_id: Uuid,
        owner_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<PostGrant, PpdcError> {
        let post = Post::find_full(post_id, pool)?;
        if post.user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }
        let grant = PostGrant::find(grant_id, pool)?;
        if grant.post_id != post_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Grant does not belong to the specified post".to_string(),
            ));
        }

        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        diesel::update(post_grants::table.filter(post_grants::id.eq(grant_id)))
            .set((
                post_grants::status.eq(PostGrantStatus::Revoked.to_db()),
                post_grants::updated_at.eq(diesel::dsl::now),
            ))
            .execute(&mut conn)?;
        PostGrant::find(grant_id, pool)
    }

    pub fn user_can_read_post(post: &Post, user_id: Uuid, pool: &DbPool) -> Result<bool, PpdcError> {
        if post.user_id == user_id {
            return Ok(true);
        }

        let grants = PostGrant::find_for_post_paginated(post.id, 0, i64::MAX / 4, pool)?.0;
        for grant in grants
            .into_iter()
            .filter(|grant| grant.status == PostGrantStatus::Active)
        {
            if grant.grantee_user_id == Some(user_id) {
                return Ok(true);
            }

            if grant.grantee_scope == Some(PostGrantScope::AllAcceptedFollowers)
                && Relationship::is_follow_accepted(user_id, post.user_id, pool)?
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn find_shared_post_ids_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        use std::collections::HashSet;

        let mut ids = HashSet::new();
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let direct_ids = post_grants::table
            .filter(post_grants::grantee_user_id.eq(Some(user_id)))
            .filter(post_grants::status.eq(PostGrantStatus::Active.to_db()))
            .select(post_grants::post_id)
            .load::<Uuid>(&mut conn)?;
        ids.extend(direct_ids);

        let follower_scope_grants = post_grants::table
            .filter(
                post_grants::grantee_scope.eq(Some(PostGrantScope::AllAcceptedFollowers.to_db())),
            )
            .filter(post_grants::status.eq(PostGrantStatus::Active.to_db()))
            .select((post_grants::post_id, post_grants::owner_user_id))
            .load::<(Uuid, Uuid)>(&mut conn)?;

        for (post_id, owner_user_id) in follower_scope_grants {
            if Relationship::is_follow_accepted(user_id, owner_user_id, pool)? {
                ids.insert(post_id);
            }
        }

        Ok(ids.into_iter().collect())
    }
}
