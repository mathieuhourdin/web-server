use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::relationships;

use super::enums::{RelationshipStatus, RelationshipType};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Relationship {
    pub id: Uuid,
    pub requester_user_id: Uuid,
    pub target_user_id: Uuid,
    pub relationship_type: RelationshipType,
    pub status: RelationshipStatus,
    pub accepted_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NewRelationshipDto {
    pub target_user_id: Uuid,
    pub relationship_type: Option<RelationshipType>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct UpdateRelationshipDto {
    pub status: RelationshipStatus,
}

type RelationshipTuple = (
    Uuid,
    Uuid,
    Uuid,
    String,
    String,
    Option<NaiveDateTime>,
    NaiveDateTime,
    NaiveDateTime,
);

fn tuple_to_relationship(row: RelationshipTuple) -> Relationship {
    let (
        id,
        requester_user_id,
        target_user_id,
        relationship_type_raw,
        status_raw,
        accepted_at,
        created_at,
        updated_at,
    ) = row;

    Relationship {
        id,
        requester_user_id,
        target_user_id,
        relationship_type: RelationshipType::from_db(&relationship_type_raw),
        status: RelationshipStatus::from_db(&status_raw),
        accepted_at,
        created_at,
        updated_at,
    }
}

fn select_relationship_columns() -> (
    relationships::id,
    relationships::requester_user_id,
    relationships::target_user_id,
    relationships::relationship_type,
    relationships::status,
    relationships::accepted_at,
    relationships::created_at,
    relationships::updated_at,
) {
    (
        relationships::id,
        relationships::requester_user_id,
        relationships::target_user_id,
        relationships::relationship_type,
        relationships::status,
        relationships::accepted_at,
        relationships::created_at,
        relationships::updated_at,
    )
}

impl Relationship {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Relationship, PpdcError> {
        let mut conn = pool.get()?;
        let row = relationships::table
            .filter(relationships::id.eq(id))
            .select(select_relationship_columns())
            .first::<RelationshipTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_relationship).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Relationship not found".to_string(),
            )
        })
    }

    pub fn find_for_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<Relationship>, PpdcError> {
        let (items, _) = Self::find_for_user_paginated(user_id, 0, i64::MAX / 4, pool)?;
        Ok(items)
    }

    pub fn find_for_user_paginated(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Relationship>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let total = relationships::table
            .filter(
                relationships::requester_user_id
                    .eq(user_id)
                    .or(relationships::target_user_id.eq(user_id)),
            )
            .count()
            .get_result::<i64>(&mut conn)?;
        let rows = relationships::table
            .filter(
                relationships::requester_user_id
                    .eq(user_id)
                    .or(relationships::target_user_id.eq(user_id)),
            )
            .select(select_relationship_columns())
            .order(relationships::updated_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<RelationshipTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_relationship).collect(), total))
    }

    pub fn find_incoming_pending_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Relationship>, PpdcError> {
        let (items, _) =
            Self::find_incoming_pending_for_user_paginated(user_id, 0, i64::MAX / 4, pool)?;
        Ok(items)
    }

    pub fn find_incoming_pending_for_user_paginated(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Relationship>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let total = relationships::table
            .filter(relationships::target_user_id.eq(user_id))
            .filter(relationships::status.eq(RelationshipStatus::Pending.to_db()))
            .count()
            .get_result::<i64>(&mut conn)?;
        let rows = relationships::table
            .filter(relationships::target_user_id.eq(user_id))
            .filter(relationships::status.eq(RelationshipStatus::Pending.to_db()))
            .select(select_relationship_columns())
            .order(relationships::updated_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<RelationshipTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_relationship).collect(), total))
    }

    pub fn find_outgoing_pending_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Relationship>, PpdcError> {
        let (items, _) =
            Self::find_outgoing_pending_for_user_paginated(user_id, 0, i64::MAX / 4, pool)?;
        Ok(items)
    }

    pub fn find_outgoing_pending_for_user_paginated(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Relationship>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let total = relationships::table
            .filter(relationships::requester_user_id.eq(user_id))
            .filter(relationships::status.eq(RelationshipStatus::Pending.to_db()))
            .count()
            .get_result::<i64>(&mut conn)?;
        let rows = relationships::table
            .filter(relationships::requester_user_id.eq(user_id))
            .filter(relationships::status.eq(RelationshipStatus::Pending.to_db()))
            .select(select_relationship_columns())
            .order(relationships::updated_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<RelationshipTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_relationship).collect(), total))
    }

    pub fn find_followers_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Relationship>, PpdcError> {
        let (items, _) = Self::find_followers_for_user_paginated(user_id, 0, i64::MAX / 4, pool)?;
        Ok(items)
    }

    pub fn find_followers_for_user_paginated(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Relationship>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let total = relationships::table
            .filter(relationships::target_user_id.eq(user_id))
            .filter(relationships::relationship_type.eq(RelationshipType::Follow.to_db()))
            .filter(relationships::status.eq(RelationshipStatus::Accepted.to_db()))
            .count()
            .get_result::<i64>(&mut conn)?;
        let rows = relationships::table
            .filter(relationships::target_user_id.eq(user_id))
            .filter(relationships::relationship_type.eq(RelationshipType::Follow.to_db()))
            .filter(relationships::status.eq(RelationshipStatus::Accepted.to_db()))
            .select(select_relationship_columns())
            .order((
                relationships::accepted_at.desc().nulls_last(),
                relationships::updated_at.desc(),
            ))
            .offset(offset)
            .limit(limit)
            .load::<RelationshipTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_relationship).collect(), total))
    }

    pub fn find_following_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Relationship>, PpdcError> {
        let (items, _) = Self::find_following_for_user_paginated(user_id, 0, i64::MAX / 4, pool)?;
        Ok(items)
    }

    pub fn find_following_for_user_paginated(
        user_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Relationship>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let total = relationships::table
            .filter(relationships::requester_user_id.eq(user_id))
            .filter(relationships::relationship_type.eq(RelationshipType::Follow.to_db()))
            .filter(relationships::status.eq(RelationshipStatus::Accepted.to_db()))
            .count()
            .get_result::<i64>(&mut conn)?;
        let rows = relationships::table
            .filter(relationships::requester_user_id.eq(user_id))
            .filter(relationships::relationship_type.eq(RelationshipType::Follow.to_db()))
            .filter(relationships::status.eq(RelationshipStatus::Accepted.to_db()))
            .select(select_relationship_columns())
            .order((
                relationships::accepted_at.desc().nulls_last(),
                relationships::updated_at.desc(),
            ))
            .offset(offset)
            .limit(limit)
            .load::<RelationshipTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_relationship).collect(), total))
    }

    pub fn find_between(
        requester_user_id: Uuid,
        target_user_id: Uuid,
        relationship_type: RelationshipType,
        pool: &DbPool,
    ) -> Result<Option<Relationship>, PpdcError> {
        let mut conn = pool.get()?;
        let row = relationships::table
            .filter(relationships::requester_user_id.eq(requester_user_id))
            .filter(relationships::target_user_id.eq(target_user_id))
            .filter(relationships::relationship_type.eq(relationship_type.to_db()))
            .select(select_relationship_columns())
            .first::<RelationshipTuple>(&mut conn)
            .optional()?;
        Ok(row.map(tuple_to_relationship))
    }

    pub fn is_follow_accepted(
        follower_user_id: Uuid,
        target_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        Ok(matches!(
            Relationship::find_between(
                follower_user_id,
                target_user_id,
                RelationshipType::Follow,
                pool,
            )?,
            Some(Relationship {
                status: RelationshipStatus::Accepted,
                ..
            })
        ))
    }
}
