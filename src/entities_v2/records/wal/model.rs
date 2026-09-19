use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Text, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalCompilation {
    pub content: String,
    pub compiled_at: NaiveDateTime,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WalCompilationViews {
    pub operational: Option<WalCompilation>,
    pub thematic: Option<WalCompilation>,
}

#[derive(Debug, Serialize)]
pub struct WalResponse {
    pub content: String,
}

#[derive(QueryableByName)]
struct WalRow {
    #[diesel(sql_type = Text)]
    wal_content: String,
    #[diesel(sql_type = Text)]
    wal_compiled: String,
}

fn views_from_json(wal_compiled: &str) -> Result<WalCompilationViews, PpdcError> {
    serde_json::from_str(wal_compiled).map_err(|error| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            format!("Failed to read WAL compilations: {error}"),
        )
    })
}

fn read_row(user_id: Uuid, pool: &DbPool) -> Result<WalRow, PpdcError> {
    let mut conn = pool.get()?;
    Ok(
        sql_query(
            "SELECT wal_content, wal_compiled::text AS wal_compiled FROM users WHERE id = $1",
        )
        .bind::<SqlUuid, _>(user_id)
        .get_result(&mut conn)?,
    )
}

pub fn get_content(user_id: Uuid, pool: &DbPool) -> Result<String, PpdcError> {
    Ok(read_row(user_id, pool)?.wal_content)
}

pub fn get_compilations(user_id: Uuid, pool: &DbPool) -> Result<WalCompilationViews, PpdcError> {
    views_from_json(&read_row(user_id, pool)?.wal_compiled)
}

pub fn replace_content(user_id: Uuid, content: String, pool: &DbPool) -> Result<(), PpdcError> {
    let mut conn = pool.get()?;
    let affected = sql_query(
        "UPDATE users
         SET wal_content = $1,
             wal_compiled = '{}'::jsonb,
             updated_at = NOW()
         WHERE id = $2",
    )
    .bind::<Text, _>(content)
    .bind::<SqlUuid, _>(user_id)
    .execute(&mut conn)?;
    if affected == 0 {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "User not found".to_string(),
        ));
    }
    Ok(())
}

pub fn append_content(user_id: Uuid, content: String, pool: &DbPool) -> Result<String, PpdcError> {
    let mut conn = pool.get()?;
    let row = sql_query(
        "UPDATE users
         SET wal_content = CASE
                 WHEN wal_content = '' THEN $1
                 WHEN RIGHT(wal_content, 1) = E'\\n' THEN wal_content || $1
                 ELSE wal_content || E'\\n' || $1
             END,
             wal_compiled = '{}'::jsonb,
             updated_at = NOW()
         WHERE id = $2
         RETURNING wal_content, wal_compiled::text AS wal_compiled",
    )
    .bind::<Text, _>(content)
    .bind::<SqlUuid, _>(user_id)
    .get_result::<WalRow>(&mut conn)
    .optional()?;
    row.map(|row| row.wal_content)
        .ok_or_else(|| PpdcError::new(404, ErrorType::ApiError, "User not found".to_string()))
}

pub fn save_compilations_if_content_matches(
    user_id: Uuid,
    source_content: &str,
    operational_content: String,
    thematic_content: String,
    pool: &DbPool,
) -> Result<WalCompilationViews, PpdcError> {
    let compiled_at = Utc::now().naive_utc();
    let compilations_json = serde_json::to_string(&WalCompilationViews {
        operational: Some(WalCompilation {
            content: operational_content,
            compiled_at,
        }),
        thematic: Some(WalCompilation {
            content: thematic_content,
            compiled_at,
        }),
    })
    .map_err(|error| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            format!("Failed to store WAL compilation: {error}"),
        )
    })?;

    let mut conn = pool.get()?;
    let row = sql_query(
        "UPDATE users
         SET wal_compiled = CAST($1 AS jsonb),
             updated_at = NOW()
         WHERE id = $2 AND wal_content = $3
         RETURNING wal_content, wal_compiled::text AS wal_compiled",
    )
    .bind::<Text, _>(compilations_json)
    .bind::<SqlUuid, _>(user_id)
    .bind::<Text, _>(source_content)
    .get_result::<WalRow>(&mut conn)
    .optional()?;
    let Some(row) = row else {
        return Err(PpdcError::new(
            409,
            ErrorType::ApiError,
            "WAL content changed while compiling; retry the compilation".to_string(),
        ));
    };
    views_from_json(&row.wal_compiled)
}
