use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::journal_grants;

use super::enums::{JournalGrantAccessLevel, JournalGrantScope, JournalGrantStatus};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JournalGrant {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub owner_user_id: Uuid,
    pub grantee_user_id: Option<Uuid>,
    pub grantee_scope: Option<JournalGrantScope>,
    pub access_level: JournalGrantAccessLevel,
    pub status: JournalGrantStatus,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NewJournalGrantDto {
    pub grantee_user_id: Option<Uuid>,
    pub grantee_scope: Option<JournalGrantScope>,
    pub access_level: Option<JournalGrantAccessLevel>,
}

type JournalGrantTuple = (
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

fn tuple_to_journal_grant(row: JournalGrantTuple) -> JournalGrant {
    let (
        id,
        journal_id,
        owner_user_id,
        grantee_user_id,
        grantee_scope_raw,
        access_level_raw,
        status_raw,
        created_at,
        updated_at,
    ) = row;

    JournalGrant {
        id,
        journal_id,
        owner_user_id,
        grantee_user_id,
        grantee_scope: grantee_scope_raw
            .as_deref()
            .and_then(JournalGrantScope::from_db),
        access_level: JournalGrantAccessLevel::from_db(&access_level_raw),
        status: JournalGrantStatus::from_db(&status_raw),
        created_at,
        updated_at,
    }
}

fn select_journal_grant_columns() -> (
    journal_grants::id,
    journal_grants::journal_id,
    journal_grants::owner_user_id,
    journal_grants::grantee_user_id,
    journal_grants::grantee_scope,
    journal_grants::access_level,
    journal_grants::status,
    journal_grants::created_at,
    journal_grants::updated_at,
) {
    (
        journal_grants::id,
        journal_grants::journal_id,
        journal_grants::owner_user_id,
        journal_grants::grantee_user_id,
        journal_grants::grantee_scope,
        journal_grants::access_level,
        journal_grants::status,
        journal_grants::created_at,
        journal_grants::updated_at,
    )
}

impl JournalGrant {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<JournalGrant, PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let row = journal_grants::table
            .filter(journal_grants::id.eq(id))
            .select(select_journal_grant_columns())
            .first::<JournalGrantTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_journal_grant).ok_or_else(|| {
            PpdcError::new(404, ErrorType::ApiError, "Journal grant not found".to_string())
        })
    }

    pub fn find_for_journal(
        journal_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<JournalGrant>, PpdcError> {
        let (items, _) = Self::find_for_journal_paginated(journal_id, 0, i64::MAX / 4, pool)?;
        Ok(items)
    }

    pub fn find_for_journal_paginated(
        journal_id: Uuid,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<JournalGrant>, i64), PpdcError> {
        let mut conn = pool.get().expect("Failed to get a connection from the pool");
        let total = journal_grants::table
            .filter(journal_grants::journal_id.eq(journal_id))
            .count()
            .get_result::<i64>(&mut conn)?;
        let rows = journal_grants::table
            .filter(journal_grants::journal_id.eq(journal_id))
            .select(select_journal_grant_columns())
            .order(journal_grants::updated_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<JournalGrantTuple>(&mut conn)?;
        Ok((rows.into_iter().map(tuple_to_journal_grant).collect(), total))
    }
}
