use super::{
    model::{ThreadListQuery, ThreadView},
    persist,
};
use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        journal::Journal,
        session::Session,
        trace::Trace,
        user::User,
    },
    pagination::{PaginatedResponse, PaginationParams},
};
use axum::extract::{Extension, Json, Path, Query};
use uuid::Uuid;
fn actor(session: &Session) -> Result<Uuid, PpdcError> {
    session.user_id.ok_or_else(PpdcError::unauthorized)
}
pub async fn list(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(q): Query<ThreadListQuery>,
) -> Result<Json<PaginatedResponse<ThreadView>>, PpdcError> {
    let p = q.pagination.validate()?;
    let (items, total) = persist::list(
        actor(&session)?,
        q.journal_id,
        None,
        &q.q,
        p.offset,
        p.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(items, p, total)))
}
pub async fn get(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<ThreadView>, PpdcError> {
    Ok(Json(persist::view(actor(&session)?, id, &pool)?))
}
pub async fn traces(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Query(q): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Trace>>, PpdcError> {
    let p = q.validate()?;
    let owner = actor(&session)?;
    let (mut items, total) = persist::traces(owner, id, p.offset, p.limit, &pool)?;
    crate::entities_v2::trace::continuation::hydrate_continuations(owner, items.iter_mut(), &pool)?;
    Ok(Json(PaginatedResponse::new(items, p, total)))
}
pub async fn for_journal(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Query(q): Query<ThreadListQuery>,
) -> Result<Json<PaginatedResponse<ThreadView>>, PpdcError> {
    let owner = actor(&session)?;
    if Journal::find_full(id, &pool)?.user_id != owner {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "Journal not found".into(),
        ));
    }
    let p = q.pagination.validate()?;
    let (items, total) = persist::list(owner, Some(id), None, &q.q, p.offset, p.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(items, p, total)))
}
pub async fn for_trace(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Query(q): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<ThreadView>>, PpdcError> {
    let owner = actor(&session)?;
    if Trace::find_full_trace(id, &pool)?.user_id != owner {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "Trace not found".into(),
        ));
    }
    let p = q.validate()?;
    let (items, total) = persist::list(owner, None, Some(id), "", p.offset, p.limit, &pool)?;
    Ok(Json(PaginatedResponse::new(items, p, total)))
}
pub async fn generate(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<ThreadView>, PpdcError> {
    let owner = actor(&session)?;
    if !User::find(&owner, &pool)?.allows_ai_features() {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "AI features are disabled for this account".into(),
        ));
    }
    Ok(Json(persist::queue(owner, id, &pool)?))
}
