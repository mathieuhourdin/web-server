use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::relationships;

use super::enums::{RelationshipStatus, RelationshipType};
use super::model::{NewRelationshipDto, Relationship};

impl Relationship {
    pub fn create_request(
        requester_user_id: Uuid,
        payload: NewRelationshipDto,
        pool: &DbPool,
    ) -> Result<Relationship, PpdcError> {
        if requester_user_id == payload.target_user_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot create a relationship with yourself".to_string(),
            ));
        }

        let relationship_type = payload
            .relationship_type
            .unwrap_or(RelationshipType::Follow);
        if let Some(existing) = Relationship::find_between(
            requester_user_id,
            payload.target_user_id,
            relationship_type,
            pool,
        )? {
            match existing.status {
                RelationshipStatus::Pending | RelationshipStatus::Accepted => return Ok(existing),
                RelationshipStatus::Blocked => {
                    return Err(PpdcError::new(
                        403,
                        ErrorType::ApiError,
                        "Relationship request is blocked".to_string(),
                    ))
                }
                RelationshipStatus::Rejected => {
                    let mut conn = pool
                        .get()
                        .expect("Failed to get a connection from the pool");
                    diesel::update(relationships::table.filter(relationships::id.eq(existing.id)))
                        .set((
                            relationships::status.eq(RelationshipStatus::Pending.to_db()),
                            relationships::accepted_at.eq::<Option<chrono::NaiveDateTime>>(None),
                            relationships::updated_at.eq(diesel::dsl::now),
                        ))
                        .execute(&mut conn)?;
                    return Relationship::find(existing.id, pool);
                }
            }
        }

        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let id = Uuid::new_v4();
        diesel::insert_into(relationships::table)
            .values((
                relationships::id.eq(id),
                relationships::requester_user_id.eq(requester_user_id),
                relationships::target_user_id.eq(payload.target_user_id),
                relationships::relationship_type.eq(relationship_type.to_db()),
                relationships::status.eq(RelationshipStatus::Pending.to_db()),
            ))
            .execute(&mut conn)?;
        Relationship::find(id, pool)
    }

    pub fn update_status(
        id: Uuid,
        actor_user_id: Uuid,
        status: RelationshipStatus,
        pool: &DbPool,
    ) -> Result<Relationship, PpdcError> {
        if status == RelationshipStatus::Pending {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot update a relationship back to pending".to_string(),
            ));
        }

        let relationship = Relationship::find(id, pool)?;
        if relationship.target_user_id != actor_user_id {
            return Err(PpdcError::new(
                403,
                ErrorType::ApiError,
                "Only the relationship target can update its status".to_string(),
            ));
        }

        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        if status == RelationshipStatus::Accepted {
            diesel::update(relationships::table.filter(relationships::id.eq(id)))
                .set((
                    relationships::status.eq(status.to_db()),
                    relationships::accepted_at.eq(diesel::dsl::sql::<
                        diesel::sql_types::Nullable<diesel::sql_types::Timestamp>,
                    >(
                        "COALESCE(accepted_at, NOW())"
                    )),
                    relationships::updated_at.eq(diesel::dsl::now),
                ))
                .execute(&mut conn)?;
        } else {
            diesel::update(relationships::table.filter(relationships::id.eq(id)))
                .set((
                    relationships::status.eq(status.to_db()),
                    relationships::updated_at.eq(diesel::dsl::now),
                ))
                .execute(&mut conn)?;
        }
        Relationship::find(id, pool)
    }
}
