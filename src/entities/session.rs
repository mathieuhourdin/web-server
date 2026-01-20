use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::schema::sessions;
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use chrono::{Duration, NaiveDateTime, Utc};
use diesel;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
}

#[derive(Deserialize, Insertable)]
#[diesel(table_name = crate::schema::sessions)]
pub struct NewSession {
    pub user_id: Option<Uuid>,
    pub token: Option<String>,
    authenticated: bool,
    pub expires_at: NaiveDateTime,
}

impl NewSession {
    pub fn new() -> NewSession {
        NewSession {
            user_id: None,
            token: None,
            authenticated: false,
            expires_at: Utc::now()
                .naive_utc()
                .checked_add_signed(Duration::days(90))
                .unwrap(),
        }
    }
}

impl Session {
    pub fn new() -> Session {
        Session {
            id: Uuid::new_v4(),
            user_id: None,
            token: None,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            expires_at: Utc::now()
                .naive_utc()
                .checked_add_signed(Duration::hours(10))
                .unwrap(),
            authenticated: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.expires_at > Utc::now().naive_utc()
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

    pub fn get_valid_session_from_id(id: &str, pool: &DbPool) -> Result<Session, PpdcError> {
        //println!("generate_valid_session_from_id id : {id}");
        let decoded_session_id = Uuid::parse_str(id);
        if decoded_session_id.is_err() {
            println!("Error when decoding session_id: {id}");
            let new_session = NewSession::new();
            return Ok(Session::create(&new_session, pool).unwrap());
        }
        let decoded_session_id = decoded_session_id.unwrap();
        let found_session = match Session::find(&decoded_session_id, pool) {
            Err(err) => {
                println!("Error when searching session for session_id: {decoded_session_id}, error : {:#?}", err);
                let new_session = NewSession::new();
                Session::create(&new_session, pool)?
            }
            Ok(session) => {
                if session.is_valid() {
                    session
                } else {
                    println!("Session {} not valid anymore", session.id);
                    let new_session = NewSession::new();
                    Session::create(&new_session, pool)?
                }
            }
        };
        //println!(
        //    "generate_valid_session_from_id found_session id {:#?}",
        //    found_session.id
        //);
        Ok(found_session)
    }
}

#[async_trait]
impl<B> FromRequestParts<B> for Session
where
    B: Send,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &B) -> Result<Self, Self::Rejection> {
        println!("from_request_parts");
        let pool = parts
            .extensions
            .get::<DbPool>()
            .expect("Extension DbPool should be set");
        println!("from_request_parts pool");
        if let Some(auth_header) = parts.headers.get("Authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                println!("from_request_parts auth_str {:#?}", auth_str);
                return Ok(Session::get_valid_session_from_id(auth_str, &pool).unwrap());
            }
        }
        Session::create(&NewSession::new(), &pool)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Unauthorized"))
    }
}
