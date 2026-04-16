use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::relationships;

use super::enums::{RelationshipStatus, RelationshipType};
use super::model::{NewRelationshipDto, Relationship};

impl Relationship {
    fn find_between_with_conn(
        requester_user_id: Uuid,
        target_user_id: Uuid,
        relationship_type: RelationshipType,
        conn: &mut diesel::PgConnection,
    ) -> Result<Option<Relationship>, PpdcError> {
        let row = relationships::table
            .filter(relationships::requester_user_id.eq(requester_user_id))
            .filter(relationships::target_user_id.eq(target_user_id))
            .filter(relationships::relationship_type.eq(relationship_type.to_db()))
            .select((
                relationships::id,
                relationships::requester_user_id,
                relationships::target_user_id,
                relationships::relationship_type,
                relationships::status,
                relationships::accepted_at,
                relationships::created_at,
                relationships::updated_at,
            ))
            .first::<(
                Uuid,
                Uuid,
                Uuid,
                String,
                String,
                Option<chrono::NaiveDateTime>,
                chrono::NaiveDateTime,
                chrono::NaiveDateTime,
            )>(conn)
            .optional()?;

        Ok(row.map(|row| Relationship {
            id: row.0,
            requester_user_id: row.1,
            target_user_id: row.2,
            relationship_type: RelationshipType::from_db(&row.3),
            status: RelationshipStatus::from_db(&row.4),
            accepted_at: row.5,
            created_at: row.6,
            updated_at: row.7,
        }))
    }

    fn ensure_auto_followback_with_conn(
        accepter_user_id: Uuid,
        requester_user_id: Uuid,
        conn: &mut diesel::PgConnection,
    ) -> Result<(), PpdcError> {
        let reverse = Self::find_between_with_conn(
            accepter_user_id,
            requester_user_id,
            RelationshipType::Follow,
            conn,
        )?;

        match reverse {
            Some(existing) => match existing.status {
                RelationshipStatus::Accepted => Ok(()),
                RelationshipStatus::Blocked => Ok(()),
                RelationshipStatus::Pending | RelationshipStatus::Rejected => {
                    diesel::update(relationships::table.filter(relationships::id.eq(existing.id)))
                        .set((
                            relationships::status.eq(RelationshipStatus::Accepted.to_db()),
                            relationships::accepted_at.eq(diesel::dsl::sql::<
                                diesel::sql_types::Nullable<diesel::sql_types::Timestamp>,
                            >(
                                "COALESCE(accepted_at, NOW())"
                            )),
                            relationships::updated_at.eq(diesel::dsl::now),
                        ))
                        .execute(conn)?;
                    Ok(())
                }
            },
            None => {
                diesel::insert_into(relationships::table)
                    .values((
                        relationships::id.eq(Uuid::new_v4()),
                        relationships::requester_user_id.eq(accepter_user_id),
                        relationships::target_user_id.eq(requester_user_id),
                        relationships::relationship_type.eq(RelationshipType::Follow.to_db()),
                        relationships::status.eq(RelationshipStatus::Accepted.to_db()),
                        relationships::accepted_at.eq(diesel::dsl::sql::<
                            diesel::sql_types::Nullable<diesel::sql_types::Timestamp>,
                        >("NOW()")),
                    ))
                    .execute(conn)?;
                Ok(())
            }
        }
    }

    pub fn create_request(
        requester_user_id: Uuid,
        payload: NewRelationshipDto,
        pool: &DbPool,
    ) -> Result<(Relationship, bool), PpdcError> {
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
                RelationshipStatus::Pending | RelationshipStatus::Accepted => {
                    return Ok((existing, false))
                }
                RelationshipStatus::Blocked => {
                    return Err(PpdcError::new(
                        403,
                        ErrorType::ApiError,
                        "Relationship request is blocked".to_string(),
                    ))
                }
                RelationshipStatus::Rejected => {
                    let mut conn = pool.get()?;
                    diesel::update(relationships::table.filter(relationships::id.eq(existing.id)))
                        .set((
                            relationships::status.eq(RelationshipStatus::Pending.to_db()),
                            relationships::accepted_at.eq::<Option<chrono::NaiveDateTime>>(None),
                            relationships::updated_at.eq(diesel::dsl::now),
                        ))
                        .execute(&mut conn)?;
                    return Ok((Relationship::find(existing.id, pool)?, true));
                }
            }
        }

        let mut conn = pool.get()?;
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
        Ok((Relationship::find(id, pool)?, true))
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

        let mut conn = pool.get()?;
        conn.transaction::<(), diesel::result::Error, _>(|conn| {
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
                    .execute(conn)?;

                if relationship.relationship_type == RelationshipType::Follow {
                    Self::ensure_auto_followback_with_conn(
                        actor_user_id,
                        relationship.requester_user_id,
                        conn,
                    )
                    .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                }
            } else {
                diesel::update(relationships::table.filter(relationships::id.eq(id)))
                    .set((
                        relationships::status.eq(status.to_db()),
                        relationships::updated_at.eq(diesel::dsl::now),
                    ))
                    .execute(conn)?;
            }
            Ok(())
        })?;
        Relationship::find(id, pool)
    }
}
