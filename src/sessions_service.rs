use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    session::Session,
    user::{User, UserPrincipalType},
};
use axum::{
    body::Body,
    debug_handler,
    extract::{Extension, Json},
    http::{HeaderMap, Request, StatusCode as AxumStatusCode},
    middleware::Next,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct LoginCheck {
    username: String,
    password: String,
}

pub async fn add_session_to_request(
    session: Session,
    mut req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    req.extensions_mut().insert(session.clone());
    next.run(req).await
}

pub async fn auth_middleware_custom(
    Extension(session): Extension<Session>,
    req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    if session.user_id.is_none() {
        return (AxumStatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    next.run(req).await
}

#[debug_handler]
pub async fn post_session_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    headers: HeaderMap,
    Json(payload): Json<LoginCheck>,
) -> Result<Json<Session>, PpdcError> {
    let existing_user = User::find_by_username(&payload.username, &pool)?;
    if existing_user.principal_type == UserPrincipalType::Service {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            String::from("Service users cannot authenticate"),
        ));
    }

    let is_valid_password = existing_user.verify_password(&payload.password.as_bytes())?;

    if is_valid_password {
        if session.user_id == Some(existing_user.id) {
            let mut session = session;
            session.token = headers
                .get("Authorization")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            return Ok(Json(session));
        }

        if session.user_id.is_some() {
            let _ = Session::revoke(session.id, &pool);
        }
        let (session, _bearer_token) = Session::create_authenticated(existing_user.id, &pool)?;
        return Ok(Json(session));
    }
    Err(PpdcError::new(
        401,
        ErrorType::ApiError,
        String::from("Invalid password"),
    ))
}

#[debug_handler]
pub async fn delete_session_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Session>, PpdcError> {
    if session.user_id.is_none() {
        return Err(PpdcError::unauthorized());
    }
    let revoked = Session::revoke(session.id, &pool)?;
    Ok(Json(revoked))
}

#[debug_handler]
pub async fn get_session_route(
    Extension(session): Extension<Session>,
) -> Result<Json<Session>, PpdcError> {
    Ok(Json(session))
}

pub fn decode_session_id(session_id: &String) -> String {
    String::from(&session_id[..])
}
