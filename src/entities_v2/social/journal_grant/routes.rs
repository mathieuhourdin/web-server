use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, journal::Journal, session::Session};

use super::model::{JournalGrant, NewJournalGrantDto};

#[debug_handler]
pub async fn get_journal_grants_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
) -> Result<Json<Vec<JournalGrant>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let grants = JournalGrant::find_for_journal(journal_id, &pool)?;
    Ok(Json(grants))
}

#[debug_handler]
pub async fn post_journal_grant_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    Json(payload): Json<NewJournalGrantDto>,
) -> Result<Json<JournalGrant>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    let grant = JournalGrant::create_or_update(&journal, user_id, payload, &pool)?;
    Ok(Json(grant))
}

#[debug_handler]
pub async fn delete_journal_grant_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((journal_id, grant_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<JournalGrant>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let grant = JournalGrant::revoke(journal_id, grant_id, user_id, &pool)?;
    Ok(Json(grant))
}
