use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::schema::sessions;
use argon2::Config;
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use chrono::{Duration, NaiveDateTime, Utc};
use diesel;
use diesel::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const BEARER_PREFIX: &str = "Bearer ";
const SESSION_TTL_DAYS: i64 = 90;
const SESSION_SECRET_BYTES: usize = 32;

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, Insertable, AsChangeset, Clone)]
#[diesel(table_name = crate::schema::sessions)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub token: Option<String>,
    authenticated: bool,
    pub expires_at: NaiveDateTime,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    #[serde(skip_serializing, skip_deserializing)]
    pub secret_hash: Option<String>,
    pub revoked_at: Option<NaiveDateTime>,
    pub last_seen_at: Option<NaiveDateTime>,
}

#[derive(Deserialize, Insertable)]
#[diesel(table_name = crate::schema::sessions)]
pub struct NewSession {
    pub user_id: Option<Uuid>,
    pub token: Option<String>,
    authenticated: bool,
    pub expires_at: NaiveDateTime,
    pub secret_hash: Option<String>,
    pub revoked_at: Option<NaiveDateTime>,
    pub last_seen_at: Option<NaiveDateTime>,
}

impl NewSession {
    pub fn new() -> NewSession {
        NewSession {
            user_id: None,
            token: None,
            authenticated: false,
            expires_at: Utc::now()
                .naive_utc()
                .checked_add_signed(Duration::days(SESSION_TTL_DAYS))
                .unwrap(),
            secret_hash: None,
            revoked_at: None,
            last_seen_at: None,
        }
    }
}

impl Session {
    pub fn anonymous() -> Session {
        Session {
            id: Uuid::new_v4(),
            user_id: None,
            token: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            expires_at: Utc::now().naive_utc(),
            authenticated: false,
            secret_hash: None,
            revoked_at: None,
            last_seen_at: None,
        }
    }

    pub fn new() -> Session {
        Session::anonymous()
    }

    pub fn is_valid(&self) -> bool {
        self.revoked_at.is_none() && self.expires_at > Utc::now().naive_utc()
    }

    pub fn set_authenticated_and_user_id(&mut self, user_id: Uuid) {
        self.user_id = Some(user_id);
        self.authenticated = true;
    }

    pub fn find(id: &Uuid, pool: &DbPool) -> Result<Session, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let session = sessions::table
            .filter(sessions::id.eq(id))
            .first(&mut conn)?;
        Ok(session)
    }

    pub fn update(session: &Session, pool: &DbPool) -> Result<Session, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let session = diesel::update(sessions::table)
            .filter(sessions::id.eq(session.id))
            .set(session)
            .get_result(&mut conn)?;
        Ok(session)
    }

    pub fn create(session: &NewSession, pool: &DbPool) -> Result<Session, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let session = diesel::insert_into(sessions::table)
            .values(session)
            .get_result(&mut conn)?;
        Ok(session)
    }

    fn parse_bearer_token(auth_value: &str) -> Option<(Uuid, String)> {
        let token = auth_value.strip_prefix(BEARER_PREFIX)?;
        let (session_id_raw, secret) = token.split_once('.')?;
        if secret.is_empty() {
            return None;
        }
        let session_id = Uuid::parse_str(session_id_raw).ok()?;
        Some((session_id, secret.to_string()))
    }

    fn parse_legacy_session_id(auth_value: &str) -> Option<Uuid> {
        Uuid::parse_str(auth_value).ok()
    }

    fn generate_secret() -> String {
        let bytes: [u8; SESSION_SECRET_BYTES] = rand::thread_rng().gen();
        bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
    }

    fn hash_secret(secret: &str) -> Result<String, PpdcError> {
        let salt: [u8; 32] = rand::thread_rng().gen();
        let config = Config::default();
        argon2::hash_encoded(secret.as_bytes(), &salt, &config).map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unable to hash session secret: {:#?}", err),
            )
        })
    }

    fn verify_secret(secret_hash: &str, candidate: &str) -> Result<bool, PpdcError> {
        argon2::verify_encoded(secret_hash, candidate.as_bytes()).map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unable to verify session secret: {:#?}", err),
            )
        })
    }

    pub fn create_authenticated(user_id: Uuid, pool: &DbPool) -> Result<(Session, String), PpdcError> {
        let secret = Session::generate_secret();
        let secret_hash = Session::hash_secret(&secret)?;
        let new_session = NewSession {
            user_id: Some(user_id),
            token: None,
            authenticated: true,
            expires_at: Utc::now()
                .naive_utc()
                .checked_add_signed(Duration::days(SESSION_TTL_DAYS))
                .unwrap(),
            secret_hash: Some(secret_hash),
            revoked_at: None,
            last_seen_at: Some(Utc::now().naive_utc()),
        };
        let mut session = Session::create(&new_session, pool)?;
        let bearer_token = format!("{}{}.{}", BEARER_PREFIX, session.id, secret);
        // Backward-compatible response channel for clients that already read `session.token`.
        session.token = Some(bearer_token.clone());
        Ok((session, bearer_token))
    }

    pub fn revoke(session_id: Uuid, pool: &DbPool) -> Result<Session, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let now = Utc::now().naive_utc();
        let session = diesel::update(sessions::table)
            .filter(sessions::id.eq(session_id))
            .set((
                sessions::revoked_at.eq(Some(now)),
                sessions::authenticated.eq(false),
                sessions::updated_at.eq(now),
            ))
            .get_result(&mut conn)?;
        Ok(session)
    }

    fn touch(session_id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let now = Utc::now().naive_utc();
        diesel::update(sessions::table)
            .filter(sessions::id.eq(session_id))
            .set((sessions::last_seen_at.eq(Some(now)), sessions::updated_at.eq(now)))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_valid_session_from_authorization(
        auth_value: &str,
        pool: &DbPool,
    ) -> Result<Session, PpdcError> {
        // Preferred secure format.
        if let Some((session_id, secret)) = Session::parse_bearer_token(auth_value) {
            let session = Session::find(&session_id, pool)?;
            if !session.is_valid() {
                return Err(PpdcError::unauthorized());
            }
            let secret_hash = session
                .secret_hash
                .as_ref()
                .ok_or_else(PpdcError::unauthorized)?;
            let matches = Session::verify_secret(secret_hash, &secret)?;
            if !matches {
                return Err(PpdcError::unauthorized());
            }
            Session::touch(session.id, pool)?;
            return Ok(session);
        }

        // Temporary compatibility fallback with old raw UUID auth header.
        if let Some(session_id) = Session::parse_legacy_session_id(auth_value) {
            let session = Session::find(&session_id, pool)?;
            if !session.is_valid() {
                return Err(PpdcError::unauthorized());
            }
            if session.secret_hash.is_some() {
                return Err(PpdcError::unauthorized());
            }
            Session::touch(session.id, pool)?;
            return Ok(session);
        }

        Err(PpdcError::unauthorized())
    }
}

#[async_trait]
impl<B> FromRequestParts<B> for Session
where
    B: Send,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &B) -> Result<Self, Self::Rejection> {
        let pool = parts
            .extensions
            .get::<DbPool>()
            .expect("Extension DbPool should be set");
        if let Some(auth_header) = parts.headers.get("Authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                if let Ok(session) = Session::get_valid_session_from_authorization(auth_str, &pool) {
                    return Ok(session);
                }
            }
        }
        Ok(Session::anonymous())
    }
}
