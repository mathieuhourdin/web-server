use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, session::Session};

use super::model::{Journal, NewJournalDto, UpdateJournalDto};

#[debug_handler]
pub async fn get_user_journals_route(
    Extension(pool): Extension<DbPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<Journal>>, PpdcError> {
    let journals = Journal::find_for_user(user_id, &pool)?;
    Ok(Json(journals))
}

#[debug_handler]
pub async fn post_journal_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewJournalDto>,
) -> Result<Json<Journal>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::create(payload, user_id, &pool)?;
    Ok(Json(journal))
}

#[debug_handler]
pub async fn put_journal_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateJournalDto>,
) -> Result<Json<Journal>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut journal = Journal::find_full(id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    if let Some(title) = payload.title {
        journal.title = title;
    }
    if let Some(subtitle) = payload.subtitle {
        journal.subtitle = subtitle;
    }
    if let Some(content) = payload.content {
        journal.content = content;
    }
    if let Some(journal_type) = payload.journal_type {
        journal.journal_type = journal_type;
    }
    if let Some(status) = payload.status {
        journal.status = status;
    }

    let journal = journal.update(&pool)?;
    Ok(Json(journal))
}
