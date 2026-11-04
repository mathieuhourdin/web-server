use super::model::{ListQuery, NewTemplate, PatchTemplate, WritingTemplate};
use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        session::Session,
        user::{User, UserRole},
    },
    pagination::PaginatedResponse,
};
use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

fn authorize(session: &Session, admin: bool, pool: &DbPool) -> Result<(), PpdcError> {
    let id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if admin && !User::find(&id, pool)?.has_role(UserRole::Admin, pool)? {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "Admin role required".into(),
        ));
    }
    Ok(())
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, PpdcError> + Send + 'static,
) -> Result<T, PpdcError> {
    tokio::task::spawn_blocking(work).await.map_err(|_| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Writing template operation failed".into(),
        )
    })?
}

pub async fn list(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(query): Query<ListQuery>,
) -> Result<Json<PaginatedResponse<WritingTemplate>>, PpdcError> {
    blocking(move || {
        authorize(&session, false, &pool)?;
        let mut conn = pool.get()?;
        Ok(Json(WritingTemplate::list(query, false, &mut conn)?))
    })
    .await
}

pub async fn get(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<WritingTemplate>, PpdcError> {
    blocking(move || {
        authorize(&session, false, &pool)?;
        let mut conn = pool.get()?;
        Ok(Json(WritingTemplate::find(id, &mut conn)?))
    })
    .await
}

pub async fn admin_list(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(query): Query<ListQuery>,
) -> Result<Json<PaginatedResponse<WritingTemplate>>, PpdcError> {
    blocking(move || {
        authorize(&session, true, &pool)?;
        let mut conn = pool.get()?;
        Ok(Json(WritingTemplate::list(query, true, &mut conn)?))
    })
    .await
}

pub async fn create(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(input): Json<NewTemplate>,
) -> Result<(StatusCode, Json<WritingTemplate>), PpdcError> {
    blocking(move || {
        authorize(&session, true, &pool)?;
        let mut conn = pool.get()?;
        Ok((StatusCode::CREATED, Json(input.create(&mut conn)?)))
    })
    .await
}

pub async fn patch(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(input): Json<PatchTemplate>,
) -> Result<Json<WritingTemplate>, PpdcError> {
    blocking(move || {
        authorize(&session, true, &pool)?;
        let mut conn = pool.get()?;
        Ok(Json(WritingTemplate::patch(id, input, &mut conn)?))
    })
    .await
}
