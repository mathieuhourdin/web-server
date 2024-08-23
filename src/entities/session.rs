use serde::{Serialize, Deserialize};
use chrono::{NaiveDateTime, Utc, Duration};
use uuid::Uuid;
use diesel::prelude::*;
use crate::db;
use crate::schema::sessions;
use diesel;
use crate::entities::error::{PpdcError};
use axum::{async_trait, http::{StatusCode, request::Parts}, extract::{FromRequestParts}};

#[derive(Debug, Serialize, Deserialize, Queryable, Selectable, Insertable, AsChangeset)]
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
            expires_at: Utc::now().naive_utc().checked_add_signed(Duration::days(90)).unwrap()
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
            expires_at: Utc::now().naive_utc().checked_add_signed(Duration::hours(10)).unwrap(),
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

    pub fn find(id: &Uuid) -> Result<Session, PpdcError> {
        let mut conn = db::establish_connection();

        let session = sessions::table
            .filter(sessions::id.eq(id))
            .first(&mut conn)?;
        Ok(session)
    }

    pub fn update(session: &Session) -> Result<Session, PpdcError> {
        let mut conn = db::establish_connection();

        let session = diesel::update(sessions::table)
            .filter(sessions::id.eq(session.id))
            .set(session)
            .get_result(&mut conn)?;
        Ok(session)
    }

    pub fn create(session: &NewSession) -> Result<Session, PpdcError> {
        let mut conn = db::establish_connection();
        let session = diesel::insert_into(sessions::table)
            .values(session)
            .get_result(&mut conn)?;
        Ok(session)
    }

    pub fn get_valid_session_from_id(id: &str) -> Result<Session, PpdcError> {

        println!("generate_valid_session_from_id id : {id}");
        let decoded_session_id = Uuid::parse_str(id)?;
        let found_session = match Session::find(&decoded_session_id) {
            Err(err) => {
                println!("Error when searching session for session_id: {decoded_session_id}, error : {:#?}", err);
                let new_session = NewSession::new();
                Session::create(&new_session)?
            },
            Ok(session) => {
                    if session.is_valid() {
                        session
                    } else {
                        println!("Session {} not valid anymore", session.id);
                        let new_session = NewSession::new();
                        Session::create(&new_session)?
                    }
                }
        };
        println!("generate_valid_session_from_id found_session id {:#?}", found_session.id);
        Ok(found_session)
    }
}

#[async_trait]
impl<B> FromRequestParts<B> for Session
where
    B: Send,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &B) -> Result<Self, Self::Rejection> {
        if let Some(auth_header) = parts.headers.get("Authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                Ok::<Session, B>(Session::get_valid_session_from_id(auth_str).unwrap());
            }
        }

        Err((StatusCode::UNAUTHORIZED, "Unauthorized"))
    }
}
