use axum::{
    extract::{Extension, Path},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use super::{model::EventInput, service};
use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        session::Session,
    },
};

pub async fn get_current(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<impl IntoResponse, PpdcError> {
    let user = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let response =
        tokio::task::spawn_blocking(move || service::current(user, chrono::Utc::now(), &pool))
            .await
            .map_err(|_| {
                PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    "Writing nudge lookup failed".into(),
                )
            })??;
    Ok((
        [(header::CACHE_CONTROL, "private, no-store")],
        Json(response),
    ))
}

pub async fn post_event(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(input): Json<EventInput>,
) -> Result<impl IntoResponse, PpdcError> {
    let user = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    tokio::task::spawn_blocking(move || {
        service::record_event(user, id, input.event, chrono::Utc::now(), &pool)
    })
    .await
    .map_err(|_| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Writing nudge tracking failed".into(),
        )
    })??;
    Ok((
        [(header::CACHE_CONTROL, "private, no-store")],
        StatusCode::NO_CONTENT,
    ))
}
