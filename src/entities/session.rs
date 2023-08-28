use serde::{Serialize, Deserialize};
use std::time::{SystemTime, Duration};
use uuid::Uuid;
use diesel::prelude::*;
use crate::db;
use crate::schema::sessions;
use diesel;
use crate::entities::error::{PpdcError};

#[derive(Serialize, Deserialize, Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::sessions)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub token: Option<String>,
    authenticated: bool,
    pub expires_at: SystemTime,
    created_at: SystemTime,
    updated_at: SystemTime,
}

#[derive(Deserialize, Insertable)]
#[diesel(table_name = crate::schema::sessions)]
pub struct NewSession {
    pub user_id: Option<Uuid>,
    pub token: Option<String>,
    authenticated: bool,
    pub expires_at: SystemTime,
}

impl NewSession {
    pub fn new() -> NewSession {
        NewSession {
            user_id: None,
            token: None,
            authenticated: false,
            expires_at: SystemTime::now() + Duration::from_secs(3600),
        }
    }
}

impl Session {
    pub fn new() -> Session {
        Session {
            id: Uuid::new_v4(),
            user_id: None,
            token: None,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            expires_at: SystemTime::now() + Duration::from_secs(3600),
            authenticated: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.expires_at > SystemTime::now()
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
}

