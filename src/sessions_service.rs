use crate::db::DbPool;
use crate::entities_v2::platform_infra::external_auth::{
    self, ExternalAuthProvider, VerifiedExternalIdentity,
};
use crate::entities_v2::{
    device::{Device, DeviceRegistrationDto},
    error::{ErrorType, PpdcError},
    session::Session,
    user::{NewUser, User, UserPrincipalType},
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
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct LoginCheck {
    username: String,
    password: String,
    device: Option<DeviceRegistrationDto>,
}

#[derive(Deserialize)]
pub struct ExternalAuthDto {
    #[serde(alias = "identity_token")]
    pub id_token: String,
    pub device: Option<DeviceRegistrationDto>,
}

#[derive(Deserialize)]
pub struct ExternalRegistrationDto {
    #[serde(alias = "identity_token")]
    pub id_token: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub handle: Option<String>,
    pub timezone: Option<String>,
    pub device: Option<DeviceRegistrationDto>,
}

#[derive(Serialize)]
pub struct ExternalRegistrationResponse {
    pub user: User,
    pub session: Session,
}

#[derive(Deserialize)]
pub struct ExternalProviderUnlinkDto {
    pub password_confirmation: Option<String>,
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

pub(crate) fn append_auth_marker_cookie(response: &mut Response<Body>) {
    append_set_cookie(response, auth_marker_cookie_value(31_536_000));
}

fn external_identity_user(
    provider: ExternalAuthProvider,
    identity: &VerifiedExternalIdentity,
    pool: &DbPool,
) -> Result<Option<User>, PpdcError> {
    match provider {
        ExternalAuthProvider::Google => User::find_by_google_sub(&identity.subject, pool),
        ExternalAuthProvider::Apple => User::find_by_apple_sub(&identity.subject, pool),
    }
}

fn link_external_identity(
    user: &User,
    identity: &VerifiedExternalIdentity,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    match identity.provider {
        ExternalAuthProvider::Google => user.link_google_sub(&identity.subject, pool),
        ExternalAuthProvider::Apple => user.link_apple_sub(&identity.subject, pool),
    }
}

fn create_external_session(
    user_id: Uuid,
    current_session: &Session,
    device: Option<DeviceRegistrationDto>,
    pool: &DbPool,
) -> Result<Session, PpdcError> {
    if current_session.user_id.is_some() {
        let _ = Session::revoke(current_session.id, pool);
    }
    let device_id = device
        .map(|device| Device::upsert_for_user(user_id, device, pool))
        .transpose()?
        .map(|device| device.id);
    let (session, _) = Session::create_authenticated_for_device(user_id, device_id, pool)?;
    Ok(session)
}

fn normalized_profile_value(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn new_external_user(
    identity: &VerifiedExternalIdentity,
    payload: &ExternalRegistrationDto,
) -> NewUser {
    let first_name = normalized_profile_value(payload.first_name.clone())
        .or_else(|| normalized_profile_value(identity.given_name.clone()))
        .unwrap_or_else(|| "Hupo".to_string());
    let last_name = normalized_profile_value(payload.last_name.clone())
        .or_else(|| normalized_profile_value(identity.family_name.clone()))
        .unwrap_or_else(|| "User".to_string());
    let handle = normalized_profile_value(payload.handle.clone())
        .map(|handle| {
            if handle.starts_with('@') {
                handle
            } else {
                format!("@{}", handle)
            }
        })
        .unwrap_or_else(|| format!("@hupo-{}", Uuid::new_v4().simple()));

    NewUser {
        email: identity.email.clone(),
        principal_type: Some(UserPrincipalType::Human),
        mentor_id: None,
        first_name,
        last_name,
        handle,
        password: Some(Uuid::new_v4().to_string()),
        profile_picture_url: None,
        profile_picture_asset_id: None,
        is_platform_user: Some(true),
        biography: None,
        pseudonym: None,
        pseudonymized: Some(false),
        high_level_projects_definition: None,
        journal_theme: None,
        current_lens_id: None,
        week_analysis_weekday: None,
        timezone: normalized_profile_value(payload.timezone.clone()).or(Some("UTC".to_string())),
        context_anchor_at: None,
        welcome_message: None,
        home_focus_view: None,
        shared_journal_activity_email_mode: None,
        received_message_email_mode: None,
        mentor_feedback_email_enabled: None,
        ai_features_enabled: None,
        ai_features_enabled_by_admin: None,
        onboarding_version: None,
        external_captures_default_journal_id: None,
        mentor_specific_prompt: None,
    }
}

async fn external_login(
    provider: ExternalAuthProvider,
    pool: DbPool,
    current_session: Session,
    payload: ExternalAuthDto,
) -> Result<Response<Body>, PpdcError> {
    let identity = external_auth::verify_id_token(provider, &payload.id_token).await?;
    let user = match external_identity_user(provider, &identity, &pool)? {
        Some(user) => user,
        None => {
            let user =
                User::find_by_email_case_insensitive(&identity.email, &pool)?.ok_or_else(|| {
                    PpdcError::new(
                        404,
                        ErrorType::ApiError,
                        "No account is linked to this external identity".to_string(),
                    )
                })?;
            link_external_identity(&user, &identity, &pool)?;
            user
        }
    };
    let session = create_external_session(user.id, &current_session, payload.device, &pool)?;
    let mut response = Json(session).into_response();
    append_auth_marker_cookie(&mut response);
    Ok(response)
}

async fn external_registration(
    provider: ExternalAuthProvider,
    pool: DbPool,
    current_session: Session,
    payload: ExternalRegistrationDto,
) -> Result<Response<Body>, PpdcError> {
    let identity = external_auth::verify_id_token(provider, &payload.id_token).await?;
    if external_identity_user(provider, &identity, &pool)?.is_some() {
        return Err(PpdcError::new(
            409,
            ErrorType::ApiError,
            "An account already exists for this external identity; use provider login".to_string(),
        ));
    }
    if User::find_by_email(&identity.email, &pool)?.is_some() {
        return Err(PpdcError::new(
            409,
            ErrorType::ApiError,
            "An account already exists for this email; sign in and link this provider".to_string(),
        ));
    }

    let mut new_user = new_external_user(&identity, &payload);
    new_user.hash_password()?;
    let user = new_user.create(&pool)?;
    if let Err(error) = link_external_identity(&user, &identity, &pool) {
        crate::entities_v2::user::cleanup_failed_user_registration(user.id, &pool);
        return Err(error);
    }
    if let Err(error) = crate::entities_v2::user::finalize_new_human_user_registration(&user, &pool)
    {
        return Err(error);
    }
    let session = create_external_session(user.id, &current_session, payload.device, &pool)?;
    let mut response = Json(ExternalRegistrationResponse { user, session }).into_response();
    append_auth_marker_cookie(&mut response);
    Ok(response)
}

async fn link_external_provider(
    provider: ExternalAuthProvider,
    pool: DbPool,
    current_session: Session,
    payload: ExternalAuthDto,
) -> Result<Json<User>, PpdcError> {
    let user_id = current_session
        .user_id
        .ok_or_else(PpdcError::unauthorized)?;
    let identity = external_auth::verify_id_token(provider, &payload.id_token).await?;
    let user = User::find(&user_id, &pool)?;
    link_external_identity(&user, &identity, &pool)?;
    Ok(Json(User::find(&user_id, &pool)?))
}

#[debug_handler]
pub async fn post_google_session_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<ExternalAuthDto>,
) -> Result<Response<Body>, PpdcError> {
    external_login(ExternalAuthProvider::Google, pool, session, payload).await
}

#[debug_handler]
pub async fn post_apple_session_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<ExternalAuthDto>,
) -> Result<Response<Body>, PpdcError> {
    external_login(ExternalAuthProvider::Apple, pool, session, payload).await
}

#[debug_handler]
pub async fn post_google_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<ExternalRegistrationDto>,
) -> Result<Response<Body>, PpdcError> {
    external_registration(ExternalAuthProvider::Google, pool, session, payload).await
}

#[debug_handler]
pub async fn post_apple_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<ExternalRegistrationDto>,
) -> Result<Response<Body>, PpdcError> {
    external_registration(ExternalAuthProvider::Apple, pool, session, payload).await
}

#[debug_handler]
pub async fn post_google_provider_link_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<ExternalAuthDto>,
) -> Result<Json<User>, PpdcError> {
    link_external_provider(ExternalAuthProvider::Google, pool, session, payload).await
}

#[debug_handler]
pub async fn post_apple_provider_link_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<ExternalAuthDto>,
) -> Result<Json<User>, PpdcError> {
    link_external_provider(ExternalAuthProvider::Apple, pool, session, payload).await
}

#[debug_handler]
pub async fn delete_google_provider_link_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<ExternalProviderUnlinkDto>,
) -> Result<Json<User>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let user = User::find(&user_id, &pool)?;
    user.unlink_google_sub(payload.password_confirmation.as_deref(), &pool)?;
    Ok(Json(User::find(&user_id, &pool)?))
}

#[debug_handler]
pub async fn delete_apple_provider_link_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<ExternalProviderUnlinkDto>,
) -> Result<Json<User>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let user = User::find(&user_id, &pool)?;
    user.unlink_apple_sub(payload.password_confirmation.as_deref(), &pool)?;
    Ok(Json(User::find(&user_id, &pool)?))
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
