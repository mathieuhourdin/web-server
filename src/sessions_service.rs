use crate::db::DbPool;
use crate::entities_v2::{
    device::{Device, DeviceRegistrationDto},
    error::{ErrorType, PpdcError},
    session::Session,
    user::{User, UserPrincipalType},
};
use crate::environment;
use axum::{
    body::Body,
    debug_handler,
    extract::{Extension, Json},
    http::{
        header::{HeaderValue, SET_COOKIE},
        HeaderMap, Request, Response, StatusCode as AxumStatusCode,
    },
    middleware::Next,
    response::IntoResponse,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct LoginCheck {
    username: String,
    password: String,
    device: Option<DeviceRegistrationDto>,
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

fn auth_marker_cookie_value(max_age_seconds: i64) -> String {
    let mut parts = vec![
        format!("hupo_user=1"),
        "Path=/".to_string(),
        format!("Max-Age={}", max_age_seconds),
        "SameSite=Lax".to_string(),
    ];
    if let Some(domain) = environment::get_auth_marker_cookie_domain() {
        parts.push(format!("Domain={}", domain));
    }
    if environment::get_env() != "development" {
        parts.push("Secure".to_string());
    }
    parts.join("; ")
}

fn auth_marker_cookie_clear_value() -> String {
    let mut parts = vec![
        "hupo_user=".to_string(),
        "Path=/".to_string(),
        "Max-Age=0".to_string(),
        "SameSite=Lax".to_string(),
    ];
    if let Some(domain) = environment::get_auth_marker_cookie_domain() {
        parts.push(format!("Domain={}", domain));
    }
    if environment::get_env() != "development" {
        parts.push("Secure".to_string());
    }
    parts.join("; ")
}

fn append_set_cookie(response: &mut Response<Body>, cookie_value: String) {
    if let Ok(header_value) = HeaderValue::from_str(&cookie_value) {
        response.headers_mut().append(SET_COOKIE, header_value);
    } else {
        tracing::warn!(
            target: "session",
            "auth_marker_cookie_invalid_header_value"
        );
    }
}

fn append_auth_marker_cookie(response: &mut Response<Body>) {
    append_set_cookie(response, auth_marker_cookie_value(31_536_000));
}

fn append_clear_auth_marker_cookie(response: &mut Response<Body>) {
    append_set_cookie(response, auth_marker_cookie_clear_value());
}

pub async fn auth_marker_cookie_middleware(
    Extension(session): Extension<Session>,
    req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    let mut response = next.run(req).await;
    if session.user_id.is_some()
        && (response.status().is_success() || response.status().is_redirection())
    {
        append_auth_marker_cookie(&mut response);
    }
    response
}

#[debug_handler]
pub async fn post_session_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    headers: HeaderMap,
    Json(payload): Json<LoginCheck>,
) -> Result<Response<Body>, PpdcError> {
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
        let device = match payload.device {
            Some(device) => Some(Device::upsert_for_user(existing_user.id, device, &pool)?),
            None => None,
        };

        if session.user_id == Some(existing_user.id) {
            if let Some(device) = device {
                if session.device_id == Some(device.id) {
                    let mut session = session;
                    session.token = headers
                        .get("Authorization")
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    let mut response = Json(session).into_response();
                    append_auth_marker_cookie(&mut response);
                    return Ok(response);
                }

                let (session, _bearer_token) = Session::create_authenticated_for_device(
                    existing_user.id,
                    Some(device.id),
                    &pool,
                )?;
                let mut response = Json(session).into_response();
                append_auth_marker_cookie(&mut response);
                return Ok(response);
            }

            let mut session = session;
            session.token = headers
                .get("Authorization")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let mut response = Json(session).into_response();
            append_auth_marker_cookie(&mut response);
            return Ok(response);
        }

        if session.user_id.is_some() {
            let _ = Session::revoke(session.id, &pool);
        }
        let device_id = device.map(|device| device.id);
        let (session, _bearer_token) =
            Session::create_authenticated_for_device(existing_user.id, device_id, &pool)?;
        let mut response = Json(session).into_response();
        append_auth_marker_cookie(&mut response);
        return Ok(response);
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
) -> Result<Response<Body>, PpdcError> {
    if session.user_id.is_none() {
        return Err(PpdcError::unauthorized());
    }
    let revoked = Session::revoke(session.id, &pool)?;
    let mut response = Json(revoked).into_response();
    append_clear_auth_marker_cookie(&mut response);
    Ok(response)
}

#[debug_handler]
pub async fn get_session_route(
    Extension(session): Extension<Session>,
) -> Result<Response<Body>, PpdcError> {
    let should_mark = session.user_id.is_some();
    let mut response = Json(session).into_response();
    if should_mark {
        append_auth_marker_cookie(&mut response);
    }
    Ok(response)
}

pub fn decode_session_id(session_id: &String) -> String {
    String::from(&session_id[..])
}
