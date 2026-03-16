use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::journal::{Journal, JournalStatus};
use crate::entities_v2::relationship::{Relationship, RelationshipStatus};
use crate::schema::journal_grants;

use super::enums::{JournalGrantAccessLevel, JournalGrantScope, JournalGrantStatus};
use super::model::{JournalGrant, NewJournalGrantDto};

impl JournalGrant {
    fn validate_target_fields(
        payload: &NewJournalGrantDto,
    ) -> Result<(Option<Uuid>, Option<JournalGrantScope>, JournalGrantAccessLevel), PpdcError> {
        match (payload.grantee_user_id, payload.grantee_scope) {
            (Some(grantee_user_id), None) => Ok((
                Some(grantee_user_id),
                None,
                payload.access_level.unwrap_or(JournalGrantAccessLevel::Read),
            )),
            (None, Some(scope)) => Ok((
                None,
                Some(scope),
                payload.access_level.unwrap_or(JournalGrantAccessLevel::Read),
            )),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Provide either grantee_user_id or grantee_scope".to_string(),
            )),
        }
    }

    fn find_existing(
        journal_id: Uuid,
        grantee_user_id: Option<Uuid>,
        grantee_scope: Option<JournalGrantScope>,
        pool: &DbPool,
    ) -> Result<Option<JournalGrant>, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let mut query = journal_grants::table
            .filter(journal_grants::journal_id.eq(journal_id))
            .into_boxed();

        if let Some(grantee_user_id) = grantee_user_id {
            query = query.filter(journal_grants::grantee_user_id.eq(Some(grantee_user_id)));
        } else if let Some(scope) = grantee_scope {
            query = query.filter(journal_grants::grantee_scope.eq(Some(scope.to_db())));
        }

        let row = query
            .select((
                journal_grants::id,
                journal_grants::journal_id,
                journal_grants::owner_user_id,
                journal_grants::grantee_user_id,
                journal_grants::grantee_scope,
                journal_grants::access_level,
                journal_grants::status,
                journal_grants::created_at,
                journal_grants::updated_at,
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
            Ok(JournalGrant {
                id: row.0,
                journal_id: row.1,
                owner_user_id: row.2,
                grantee_user_id: row.3,
                grantee_scope: row.4.as_deref().and_then(JournalGrantScope::from_db),
                access_level: JournalGrantAccessLevel::from_db(&row.5),
                status: JournalGrantStatus::from_db(&row.6),
                created_at: row.7,
                updated_at: row.8,
            })
        })
        .transpose()
    }

    pub fn create_or_update(
        journal: &Journal,
        owner_user_id: Uuid,
        payload: NewJournalGrantDto,
        pool: &DbPool,
    ) -> Result<JournalGrant, PpdcError> {
        if journal.user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }
        if journal.is_encrypted {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Encrypted journals cannot be shared".to_string(),
            ));
        }

        let (grantee_user_id, grantee_scope, access_level) =
            JournalGrant::validate_target_fields(&payload)?;

        if let Some(grantee_user_id) = grantee_user_id {
            if grantee_user_id == owner_user_id {
                return Err(PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Cannot grant a journal to yourself".to_string(),
                ));
            }
            let accepted = Relationship::is_follow_accepted(grantee_user_id, owner_user_id, pool)?;
            if !accepted {
                return Err(PpdcError::new(
                    403,
                    ErrorType::ApiError,
                    "Direct journal grants are restricted to accepted followers".to_string(),
                ));
            }
        }

        if let Some(existing) =
            JournalGrant::find_existing(journal.id, grantee_user_id, grantee_scope, pool)?
        {
            let mut conn = pool.get().expect("Failed to get a connection from the pool");
            diesel::update(journal_grants::table.filter(journal_grants::id.eq(existing.id)))
                .set((
                    journal_grants::access_level.eq(access_level.to_db()),
                    journal_grants::status.eq(JournalGrantStatus::Active.to_db()),
                    journal_grants::updated_at.eq(diesel::dsl::now),
                ))
                .execute(&mut conn)?;
            return JournalGrant::find(existing.id, pool);
        }

        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let id = Uuid::new_v4();
        diesel::insert_into(journal_grants::table)
            .values((
                journal_grants::id.eq(id),
                journal_grants::journal_id.eq(journal.id),
                journal_grants::owner_user_id.eq(owner_user_id),
                journal_grants::grantee_user_id.eq(grantee_user_id),
                journal_grants::grantee_scope.eq(grantee_scope.map(|scope| scope.to_db())),
                journal_grants::access_level.eq(access_level.to_db()),
                journal_grants::status.eq(JournalGrantStatus::Active.to_db()),
            ))
            .execute(&mut conn)?;
        JournalGrant::find(id, pool)
    }

    pub fn revoke(
        journal_id: Uuid,
        grant_id: Uuid,
        owner_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<JournalGrant, PpdcError> {
        let journal = Journal::find_full(journal_id, pool)?;
        if journal.user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }
        let grant = JournalGrant::find(grant_id, pool)?;
        if grant.journal_id != journal_id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Grant does not belong to the specified journal".to_string(),
            ));
        }

        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        diesel::update(journal_grants::table.filter(journal_grants::id.eq(grant_id)))
            .set((
                journal_grants::status.eq(JournalGrantStatus::Revoked.to_db()),
                journal_grants::updated_at.eq(diesel::dsl::now),
            ))
            .execute(&mut conn)?;
        JournalGrant::find(grant_id, pool)
    }

    pub fn user_can_read_journal(
        journal: &Journal,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        if journal.user_id == user_id {
            return Ok(true);
        }
        if journal.status == JournalStatus::Archived {
            return Ok(false);
        }
        if journal.is_encrypted {
            return Ok(false);
        }

        let grants = JournalGrant::find_for_journal(journal.id, pool)?;
        for grant in grants
            .into_iter()
            .filter(|grant| grant.status == JournalGrantStatus::Active)
        {
            if grant.grantee_user_id == Some(user_id) {
                return Ok(true);
            }

            if grant.grantee_scope == Some(JournalGrantScope::AllAcceptedFollowers)
                && matches!(
                    Relationship::find_between(
                        user_id,
                        journal.user_id,
                        crate::entities_v2::relationship::RelationshipType::Follow,
                        pool,
                    )?,
                    Some(Relationship {
                        status: RelationshipStatus::Accepted,
                        ..
                    })
                )
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn find_shared_journal_ids_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        use std::collections::HashSet;

        let mut ids = HashSet::new();
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let direct_ids = journal_grants::table
            .filter(journal_grants::grantee_user_id.eq(Some(user_id)))
            .filter(journal_grants::status.eq(JournalGrantStatus::Active.to_db()))
            .select(journal_grants::journal_id)
            .load::<Uuid>(&mut conn)?;
        ids.extend(direct_ids);

        let follower_scope_grants = journal_grants::table
            .filter(
                journal_grants::grantee_scope
                    .eq(Some(JournalGrantScope::AllAcceptedFollowers.to_db())),
            )
            .filter(journal_grants::status.eq(JournalGrantStatus::Active.to_db()))
            .select((journal_grants::journal_id, journal_grants::owner_user_id))
            .load::<(Uuid, Uuid)>(&mut conn)?;

        for (journal_id, owner_user_id) in follower_scope_grants {
            if Relationship::is_follow_accepted(user_id, owner_user_id, pool)? {
                ids.insert(journal_id);
            }
        }

        Ok(ids.into_iter().collect())
    }

    pub fn find_active_recipient_user_ids_for_journal(
        journal: &Journal,
        pool: &DbPool,
    ) -> Result<Vec<Uuid>, PpdcError> {
        use std::collections::HashSet;

        let mut ids = HashSet::new();
        let grants = JournalGrant::find_for_journal(journal.id, pool)?;

        for grant in grants
            .into_iter()
            .filter(|grant| grant.status == JournalGrantStatus::Active)
        {
            if let Some(grantee_user_id) = grant.grantee_user_id {
                ids.insert(grantee_user_id);
            }

            if grant.grantee_scope == Some(JournalGrantScope::AllAcceptedFollowers) {
                for relationship in Relationship::find_followers_for_user(journal.user_id, pool)? {
                    ids.insert(relationship.requester_user_id);
                }
            }
        }

        ids.remove(&journal.user_id);
        Ok(ids.into_iter().collect())
    }

    pub fn has_active_grants_for_journal(
        journal_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let count = journal_grants::table
            .filter(journal_grants::journal_id.eq(journal_id))
            .filter(journal_grants::status.eq(JournalGrantStatus::Active.to_db()))
            .count()
            .get_result::<i64>(&mut conn)?;
        Ok(count > 0)
    }
}
