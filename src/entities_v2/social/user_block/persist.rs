use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    post_grant::PostGrant,
};
use crate::schema::{journal_sharing_policies, relationships, user_blocks};

use super::model::UserBlock;

impl UserBlock {
    pub fn ensure_can_interact(
        user_a_id: Uuid,
        user_b_id: Uuid,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        if user_a_id != user_b_id && Self::exists_in_either_direction(user_a_id, user_b_id, pool)? {
            return Err(PpdcError::new(
                404,
                ErrorType::ApiError,
                "Resource not found".to_string(),
            ));
        }
        Ok(())
    }

    pub fn exists_in_either_direction(
        user_a_id: Uuid,
        user_b_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let mut conn = pool.get()?;
        Ok(user_blocks::table
            .filter(
                user_blocks::blocker_user_id
                    .eq(user_a_id)
                    .and(user_blocks::blocked_user_id.eq(user_b_id))
                    .or(user_blocks::blocker_user_id
                        .eq(user_b_id)
                        .and(user_blocks::blocked_user_id.eq(user_a_id))),
            )
            .select(user_blocks::blocker_user_id)
            .first::<Uuid>(&mut conn)
            .optional()?
            .is_some())
    }

    pub fn blocked_user_ids_in_either_direction(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        let mut conn = pool.get()?;
        let rows = user_blocks::table
            .filter(
                user_blocks::blocker_user_id
                    .eq(user_id)
                    .or(user_blocks::blocked_user_id.eq(user_id)),
            )
            .select((user_blocks::blocker_user_id, user_blocks::blocked_user_id))
            .load::<(Uuid, Uuid)>(&mut conn)?;
        Ok(rows
            .into_iter()
            .map(|(blocker_user_id, blocked_user_id)| {
                if blocker_user_id == user_id {
                    blocked_user_id
                } else {
                    blocker_user_id
                }
            })
            .collect())
    }

    pub fn find_blocked_user_ids_paginated(
        blocker_user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Uuid>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let total = user_blocks::table
            .filter(user_blocks::blocker_user_id.eq(blocker_user_id))
            .count()
            .get_result::<i64>(&mut conn)?;
        let ids = user_blocks::table
            .filter(user_blocks::blocker_user_id.eq(blocker_user_id))
            .order(user_blocks::created_at.desc())
            .offset(offset)
            .limit(limit)
            .select(user_blocks::blocked_user_id)
            .load::<Uuid>(&mut conn)?;
        Ok((ids, total))
    }

    pub fn block_user(
        blocker_user_id: Uuid,
        blocked_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        if blocker_user_id == blocked_user_id {
            return Err(PpdcError::new(
                422,
                ErrorType::ApiError,
                "Cannot block yourself".to_string(),
            ));
        }

        let mut conn = pool.get()?;
        conn.transaction::<(), PpdcError, _>(|conn| {
            diesel::insert_into(user_blocks::table)
                .values((
                    user_blocks::blocker_user_id.eq(blocker_user_id),
                    user_blocks::blocked_user_id.eq(blocked_user_id),
                ))
                .on_conflict((user_blocks::blocker_user_id, user_blocks::blocked_user_id))
                .do_nothing()
                .execute(conn)?;

            diesel::delete(relationships::table.filter(
                relationships::requester_user_id
                    .eq(blocker_user_id)
                    .and(relationships::target_user_id.eq(blocked_user_id))
                    .or(relationships::requester_user_id
                        .eq(blocked_user_id)
                        .and(relationships::target_user_id.eq(blocker_user_id))),
            ))
            .execute(conn)?;

            PostGrant::revoke_all_direct_between_users_with_conn(
                blocker_user_id,
                blocked_user_id,
                conn,
            )?;

            diesel::update(journal_sharing_policies::table.filter(
                journal_sharing_policies::owner_user_id
                    .eq(blocker_user_id)
                    .and(journal_sharing_policies::grantee_user_id.eq(blocked_user_id))
                    .or(journal_sharing_policies::owner_user_id
                        .eq(blocked_user_id)
                        .and(journal_sharing_policies::grantee_user_id.eq(blocker_user_id))),
            ))
            .set((
                journal_sharing_policies::status.eq("REVOKED"),
                journal_sharing_policies::default_future_access_enabled.eq(false),
                journal_sharing_policies::updated_at.eq(diesel::dsl::now),
            ))
            .execute(conn)?;
            Ok(())
        })
    }

    pub fn unblock_user(
        blocker_user_id: Uuid,
        blocked_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        let mut conn = pool.get()?;
        diesel::delete(
            user_blocks::table
                .filter(user_blocks::blocker_user_id.eq(blocker_user_id))
                .filter(user_blocks::blocked_user_id.eq(blocked_user_id)),
        )
        .execute(&mut conn)?;
        Ok(())
    }
}
