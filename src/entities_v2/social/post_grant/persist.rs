use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::post::Post;
use crate::entities_v2::relationship::Relationship;
use crate::entities_v2::user::{User, UserPrincipalType, UserRole};
use crate::schema::{post_grants, posts, users};

use super::enums::{PostGrantAccessLevel, PostGrantScope, PostGrantStatus};
use super::model::{NewPostGrantDto, PostGrant};

impl PostGrant {
    pub fn find_visible_post_ids_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        use std::collections::HashSet;

        let mut conn = pool.get()?;
        let direct_ids = post_grants::table
            .filter(post_grants::grantee_user_id.eq(Some(user_id)))
            .filter(post_grants::status.eq(PostGrantStatus::Active.to_db()))
            .select(post_grants::post_id)
            .load::<Uuid>(&mut conn)?;
        let mut candidate_ids = direct_ids.iter().copied().collect::<HashSet<_>>();

        let follower_scope_grants = post_grants::table
            .filter(
                post_grants::grantee_scope.eq(Some(PostGrantScope::AllAcceptedFollowers.to_db())),
            )
            .filter(post_grants::status.eq(PostGrantStatus::Active.to_db()))
            .select((post_grants::post_id, post_grants::owner_user_id))
            .load::<(Uuid, Uuid)>(&mut conn)?;

        for (post_id, owner_user_id) in follower_scope_grants {
            if Relationship::is_follow_accepted(user_id, owner_user_id, pool)? {
                candidate_ids.insert(post_id);
            }
        }

        let platform_scope_post_ids = post_grants::table
            .filter(post_grants::grantee_scope.eq(Some(PostGrantScope::AllPlatformUsers.to_db())))
            .filter(post_grants::status.eq(PostGrantStatus::Active.to_db()))
            .select(post_grants::post_id)
            .load::<Uuid>(&mut conn)?;

        let user = User::find(&user_id, pool)?;
        if user.is_platform_user && user.principal_type == UserPrincipalType::Human {
            candidate_ids.extend(platform_scope_post_ids);
        }

        Ok(candidate_ids.into_iter().collect())
    }

    fn owner_can_use_scope(
        owner_user_id: Uuid,
        grantee_scope: Option<PostGrantScope>,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        if grantee_scope != Some(PostGrantScope::AllPlatformUsers) {
            return Ok(());
        }

        let owner = User::find(&owner_user_id, pool)?;
        if !owner.has_role(UserRole::Admin, pool)? {
            return Err(PpdcError::new(
                403,
                ErrorType::ApiError,
                "ALL_PLATFORM_USERS post grants are restricted to admins".to_string(),
            ));
        }

        Ok(())
    }

    fn list_platform_human_user_ids(pool: &DbPool) -> Result<Vec<Uuid>, PpdcError> {
        let mut conn = pool.get()?;

        let ids = users::table
            .filter(users::is_platform_user.eq(true))
            .filter(users::principal_type.eq(UserPrincipalType::Human))
            .select(users::id)
            .load::<Uuid>(&mut conn)?;
        Ok(ids)
    }

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
        let mut conn = pool.get()?;
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
        Self::owner_can_use_scope(owner_user_id, grantee_scope, pool)?;

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
            let mut conn = pool.get()?;
            diesel::update(post_grants::table.filter(post_grants::id.eq(existing.id)))
                .set((
                    post_grants::access_level.eq(access_level.to_db()),
                    post_grants::status.eq(PostGrantStatus::Active.to_db()),
                    post_grants::updated_at.eq(diesel::dsl::now),
                ))
                .execute(&mut conn)?;
            return PostGrant::find(existing.id, pool);
        }

        let mut conn = pool.get()?;
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

        let mut conn = pool.get()?;
        diesel::update(post_grants::table.filter(post_grants::id.eq(grant_id)))
            .set((
                post_grants::status.eq(PostGrantStatus::Revoked.to_db()),
                post_grants::updated_at.eq(diesel::dsl::now),
            ))
            .execute(&mut conn)?;
        PostGrant::find(grant_id, pool)
    }

    pub fn user_can_read_post(
        post: &Post,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
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

            if grant.grantee_scope == Some(PostGrantScope::AllPlatformUsers) {
                let user = User::find(&user_id, pool)?;
                if user.is_platform_user && user.principal_type == UserPrincipalType::Human {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    pub fn find_shared_post_ids_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        use chrono::NaiveDateTime;
        use std::collections::{HashMap, HashSet};

        #[derive(Clone, Copy)]
        struct VisiblePostCandidate {
            post_id: Uuid,
            specificity: i32,
            published_or_created_at: NaiveDateTime,
            created_at: NaiveDateTime,
        }

        let mut conn = pool.get()?;
        let direct_ids = post_grants::table
            .filter(post_grants::grantee_user_id.eq(Some(user_id)))
            .filter(post_grants::status.eq(PostGrantStatus::Active.to_db()))
            .select(post_grants::post_id)
            .load::<Uuid>(&mut conn)?;
        let direct_ids_set = direct_ids.iter().copied().collect::<HashSet<_>>();
        let candidate_ids = Self::find_visible_post_ids_for_user(user_id, pool)?
            .into_iter()
            .collect::<HashSet<_>>();

        if candidate_ids.is_empty() {
            return Ok(vec![]);
        }

        let candidate_rows = posts::table
            .filter(posts::id.eq_any(candidate_ids.iter().copied().collect::<Vec<_>>()))
            .select((
                posts::id,
                posts::source_trace_id,
                posts::publishing_date,
                posts::created_at,
            ))
            .load::<(Uuid, Option<Uuid>, Option<NaiveDateTime>, NaiveDateTime)>(&mut conn)?;

        let mut selected_ids = HashSet::new();
        let mut best_by_trace_id = HashMap::<Uuid, VisiblePostCandidate>::new();

        for (post_id, source_trace_id, publishing_date, created_at) in candidate_rows {
            let candidate = VisiblePostCandidate {
                post_id,
                specificity: if direct_ids_set.contains(&post_id) {
                    2
                } else {
                    1
                },
                published_or_created_at: publishing_date.unwrap_or(created_at),
                created_at,
            };

            if let Some(source_trace_id) = source_trace_id {
                let should_replace = best_by_trace_id
                    .get(&source_trace_id)
                    .map(|current| {
                        candidate.specificity > current.specificity
                            || (candidate.specificity == current.specificity
                                && (candidate.published_or_created_at
                                    > current.published_or_created_at
                                    || (candidate.published_or_created_at
                                        == current.published_or_created_at
                                        && candidate.created_at > current.created_at)))
                    })
                    .unwrap_or(true);

                if should_replace {
                    best_by_trace_id.insert(source_trace_id, candidate);
                }
            } else {
                selected_ids.insert(post_id);
            }
        }

        selected_ids.extend(
            best_by_trace_id
                .into_values()
                .map(|candidate| candidate.post_id),
        );
        Ok(selected_ids.into_iter().collect())
    }

    pub fn find_active_recipient_user_ids_for_post(
        post: &Post,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        use std::collections::HashSet;

        let mut ids = HashSet::new();
        let grants = PostGrant::find_for_post_paginated(post.id, 0, i64::MAX / 4, pool)?.0;

        for grant in grants
            .into_iter()
            .filter(|grant| grant.status == PostGrantStatus::Active)
        {
            if let Some(grantee_user_id) = grant.grantee_user_id {
                ids.insert(grantee_user_id);
            }

            if grant.grantee_scope == Some(PostGrantScope::AllAcceptedFollowers) {
                for relationship in Relationship::find_followers_for_user(post.user_id, pool)? {
                    ids.insert(relationship.requester_user_id);
                }
            }

            if grant.grantee_scope == Some(PostGrantScope::AllPlatformUsers) {
                ids.extend(Self::list_platform_human_user_ids(pool)?);
            }
        }

        ids.remove(&post.user_id);
        Ok(ids.into_iter().collect())
    }
}
