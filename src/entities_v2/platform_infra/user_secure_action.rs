use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    platform_infra::mailer::{self, NewOutboundEmail, OutboundEmailProvider},
    session::Session,
    user::{User, UserPrincipalType},
};
use crate::environment;
use crate::schema::{sessions, user_secure_actions, users};
use argon2::Config;
use axum::{
    debug_handler,
    extract::{Extension, Json},
};
use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use rand::Rng;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

const PASSWORD_RESET_TTL_MINUTES: i64 = 30;
const ACTION_SECRET_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserSecureActionType {
    PasswordReset,
}

impl UserSecureActionType {
    pub fn to_db(self) -> &'static str {
        match self {
            UserSecureActionType::PasswordReset => "PASSWORD_RESET",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, PpdcError> {
        match value {
            "PASSWORD_RESET" | "password_reset" => Ok(UserSecureActionType::PasswordReset),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid user secure action type: {}", value),
            )),
        }
    }

    pub fn to_api_value(self) -> &'static str {
        match self {
            UserSecureActionType::PasswordReset => "password_reset",
        }
    }
}

impl Serialize for UserSecureActionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_api_value())
    }
}

impl<'de> Deserialize<'de> for UserSecureActionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        UserSecureActionType::from_db(&value)
            .map_err(|_| de::Error::custom("unknown user secure action type"))
    }
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = crate::schema::user_secure_actions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserSecureAction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub action_type: String,
    pub payload: Option<String>,
    pub secret_hash: String,
    pub expires_at: NaiveDateTime,
    pub used_at: Option<NaiveDateTime>,
    pub revoked_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::user_secure_actions)]
pub struct NewUserSecureAction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub action_type: String,
    pub payload: Option<String>,
    pub secret_hash: String,
    pub expires_at: NaiveDateTime,
    pub used_at: Option<NaiveDateTime>,
    pub revoked_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize)]
pub struct CreateUserSecureActionDto {
    pub action_type: UserSecureActionType,
    pub email: Option<String>,
}

#[derive(Deserialize)]
pub struct ConsumeUserSecureActionDto {
    pub action_type: UserSecureActionType,
    pub token: String,
    pub new_password: Option<String>,
}

#[derive(Serialize)]
pub struct UserSecureActionResponse {
    pub message: String,
}

#[derive(Serialize)]
pub struct ConsumeUserSecureActionResponse {
    pub session: Session,
}

impl UserSecureAction {
    fn action_type_enum(&self) -> Result<UserSecureActionType, PpdcError> {
        UserSecureActionType::from_db(&self.action_type)
    }

    fn is_available(&self) -> bool {
        self.used_at.is_none()
            && self.revoked_at.is_none()
            && self.expires_at > Utc::now().naive_utc()
    }

    fn parse_token(token: &str) -> Option<(Uuid, String)> {
        let (action_id_raw, secret) = token.split_once('.')?;
        if secret.is_empty() {
            return None;
        }
        let action_id = Uuid::parse_str(action_id_raw).ok()?;
        Some((action_id, secret.to_string()))
    }

    fn generate_secret() -> String {
        let bytes: [u8; ACTION_SECRET_BYTES] = rand::thread_rng().gen();
        bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
    }

    fn hash_secret(secret: &str) -> Result<String, PpdcError> {
        let salt: [u8; 32] = rand::thread_rng().gen();
        let config = Config::default();
        argon2::hash_encoded(secret.as_bytes(), &salt, &config).map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unable to hash secure action secret: {:#?}", err),
            )
        })
    }

    fn verify_secret(secret_hash: &str, candidate: &str) -> Result<bool, PpdcError> {
        argon2::verify_encoded(secret_hash, candidate.as_bytes()).map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unable to verify secure action secret: {:#?}", err),
            )
        })
    }

    fn create_password_reset(user_id: Uuid, pool: &DbPool) -> Result<String, PpdcError> {
        let mut conn = pool.get()?;
        let now = Utc::now().naive_utc();
        let secret = Self::generate_secret();
        let secret_hash = Self::hash_secret(&secret)?;
        let action = conn.transaction::<UserSecureAction, diesel::result::Error, _>(|conn| {
            diesel::update(user_secure_actions::table)
                .filter(user_secure_actions::user_id.eq(user_id))
                .filter(
                    user_secure_actions::action_type
                        .eq(UserSecureActionType::PasswordReset.to_db()),
                )
                .filter(user_secure_actions::used_at.is_null())
                .filter(user_secure_actions::revoked_at.is_null())
                .set((
                    user_secure_actions::revoked_at.eq(Some(now)),
                    user_secure_actions::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(user_secure_actions::table)
                .values(&NewUserSecureAction {
                    id: Uuid::new_v4(),
                    user_id,
                    action_type: UserSecureActionType::PasswordReset.to_db().to_string(),
                    payload: None,
                    secret_hash,
                    expires_at: now
                        .checked_add_signed(Duration::minutes(PASSWORD_RESET_TTL_MINUTES))
                        .unwrap(),
                    used_at: None,
                    revoked_at: None,
                    created_at: now,
                    updated_at: now,
                })
                .returning(UserSecureAction::as_returning())
                .get_result(conn)
        })?;

        Ok(format!("{}.{}", action.id, secret))
    }

    fn consume_password_reset(
        token: &str,
        new_password: &str,
        pool: &DbPool,
    ) -> Result<Uuid, PpdcError> {
        let (action_id, secret) = Self::parse_token(token).ok_or_else(|| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                "Invalid or expired password reset token".to_string(),
            )
        })?;

        let mut conn = pool.get()?;
        let action = user_secure_actions::table
            .filter(user_secure_actions::id.eq(action_id))
            .select(UserSecureAction::as_select())
            .first::<UserSecureAction>(&mut conn)
            .optional()?
            .ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "Invalid or expired password reset token".to_string(),
                )
            })?;

        if action.action_type_enum()? != UserSecureActionType::PasswordReset
            || !action.is_available()
        {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Invalid or expired password reset token".to_string(),
            ));
        }

        let secret_matches = Self::verify_secret(&action.secret_hash, &secret)?;
        if !secret_matches {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Invalid or expired password reset token".to_string(),
            ));
        }

        let hashed_password = User::hash_password_value(new_password)?;
        let now = Utc::now().naive_utc();
        conn.transaction::<(), diesel::result::Error, _>(|conn| {
            diesel::update(users::table.filter(users::id.eq(action.user_id)))
                .set((
                    users::password.eq(hashed_password.clone()),
                    users::updated_at.eq(Some(now)),
                ))
                .execute(conn)?;

            diesel::update(
                user_secure_actions::table.filter(user_secure_actions::id.eq(action.id)),
            )
            .set((
                user_secure_actions::used_at.eq(Some(now)),
                user_secure_actions::updated_at.eq(now),
            ))
            .execute(conn)?;

            diesel::update(sessions::table.filter(sessions::user_id.eq(Some(action.user_id))))
                .set((
                    sessions::revoked_at.eq(Some(now)),
                    sessions::authenticated.eq(false),
                    sessions::updated_at.eq(now),
                ))
                .execute(conn)?;

            Ok(())
        })?;

        Ok(action.user_id)
    }
}

fn find_password_reset_user_by_email(
    email: &str,
    pool: &DbPool,
) -> Result<Option<User>, PpdcError> {
    let mut conn = pool.get()?;
    users::table
        .filter(users::email.eq(email))
        .filter(users::principal_type.eq(UserPrincipalType::Human))
        .filter(users::is_platform_user.eq(true))
        .select(User::as_select())
        .first::<User>(&mut conn)
        .optional()
        .map_err(PpdcError::from)
}

fn enqueue_password_reset_email(
    user: &User,
    token: &str,
    pool: &DbPool,
) -> Result<Uuid, PpdcError> {
    let app_base_url = environment::get_app_base_url();
    let reset_url = format!(
        "{}/reset-password?token={}",
        app_base_url.trim_end_matches('/'),
        token
    );
    let template = mailer::password_reset_email(&user.display_name(), &reset_url);
    let email = NewOutboundEmail::new(
        Some(user.id),
        "PASSWORD_RESET".to_string(),
        Some("USER_SECURE_ACTION".to_string()),
        None,
        user.email.clone(),
        "Matiere Grise <noreply@ppdcoeur.fr>".to_string(),
        template.subject,
        template.text_body,
        template.html_body,
        OutboundEmailProvider::Resend,
        Some(Utc::now().naive_utc()),
    )
    .create(pool)?;
    Ok(email.id)
}

#[debug_handler]
pub async fn post_user_secure_action_route(
    Extension(pool): Extension<DbPool>,
    Json(payload): Json<CreateUserSecureActionDto>,
) -> Result<Json<UserSecureActionResponse>, PpdcError> {
    match payload.action_type {
        UserSecureActionType::PasswordReset => {
            let email = payload.email.ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "email is required for password_reset".to_string(),
                )
            })?;

            if let Some(user) = find_password_reset_user_by_email(&email, &pool)? {
                let token = UserSecureAction::create_password_reset(user.id, &pool)?;
                let email_id = enqueue_password_reset_email(&user, &token, &pool)?;
                let pool_for_task = pool.clone();
                tokio::spawn(async move {
                    let _ = mailer::process_pending_emails(vec![email_id], &pool_for_task).await;
                });
            }

            Ok(Json(UserSecureActionResponse {
                message: "If an account exists for this email, a recovery email has been sent."
                    .to_string(),
            }))
        }
    }
}

#[debug_handler]
pub async fn post_user_secure_action_consume_route(
    Extension(pool): Extension<DbPool>,
    Json(payload): Json<ConsumeUserSecureActionDto>,
) -> Result<Json<ConsumeUserSecureActionResponse>, PpdcError> {
    match payload.action_type {
        UserSecureActionType::PasswordReset => {
            let new_password = payload.new_password.ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "new_password is required for password_reset".to_string(),
                )
            })?;
            let user_id =
                UserSecureAction::consume_password_reset(&payload.token, &new_password, &pool)?;
            let (session, _bearer_token) = Session::create_authenticated(user_id, &pool)?;
            Ok(Json(ConsumeUserSecureActionResponse { session }))
        }
    }
}
