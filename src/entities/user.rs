use serde::{Serialize, Deserialize};
use argon2::{Config};
use rand::Rng;
use crate::entities::error::{PpdcError, ErrorType};
use crate::http::{HttpRequest, HttpResponse, StatusCode};
use diesel::prelude::*;
use uuid::Uuid;
use crate::db;
use crate::schema::users;
use serde_json;
use diesel;
use chrono::NaiveDateTime;

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::users)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub handle: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
    pub profile_picture_url: Option<String>
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::users)]
pub struct NewUser {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub handle: String,
    pub password: Option<String>,
    pub profile_picture_url: Option<String>
}

impl NewUser {
    pub fn create(self) -> Result<User, PpdcError> {
        let mut conn = db::establish_connection();
        //let user = User::from(user);
        let user = diesel::insert_into(users::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(user)
    }

    pub fn update(self, id: &Uuid) -> Result<User, PpdcError> {
        let mut conn = db::establish_connection();

        let result = diesel::update(users::table)
            .filter(users::id.eq(id))
            .set(&self)
            .get_result(&mut conn)?;
        Ok(result)
    }

    pub fn hash_password(&mut self) -> Result<(), PpdcError> {
        let salt: [u8; 32] = rand::thread_rng().gen();
        let config = Config::default();

        if let Some(password) = &self.password {
        self.password = Some(argon2::hash_encoded(password.as_bytes(), &salt, &config)
            .map_err(|err| PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!("Unable to encode password: {:#?}", err)))?);
        Ok(())
        } else {
            Err(PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("No password to encode")))
        }
    }
}

impl User {
    pub fn verify_password(&self, tested_password_bytes: &[u8]) -> Result<bool, PpdcError> {
        argon2::verify_encoded(&self.password, tested_password_bytes).map_err(|err| PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unable to decode password: {:#?}", err)))
    }

    pub fn find(id: &Uuid) -> Result<User, PpdcError> {
        let mut conn = db::establish_connection();

        let user = users::table
            .filter(users::id.eq(id))
            .first(&mut conn)?;
        Ok(user)
    }

    pub fn find_by_username(email: &String) -> Result<User, PpdcError> {
        let mut conn = db::establish_connection();
        let user = users::table
            .filter(users::email.eq(email))
            .first(&mut conn)?;
        Ok(user)
    }
}

pub fn get_users(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let (offset, limit) = request.get_pagination();
    let mut conn = db::establish_connection();

    let results = users::table.into_boxed()
        .offset(offset)
        .limit(limit)
        .load::<User>(&mut conn)?;
    HttpResponse::ok()
        .json(&results)
}

pub fn post_user(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let mut user_message = serde_json::from_str::<NewUser>(&request.body[..])?;
    user_message.hash_password().unwrap();
    let created_user = user_message.create()?;
    HttpResponse::created()
        .json(&created_user)
}

pub fn put_user_route(id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let id = HttpRequest::parse_uuid(id)?;
    if let Some(user_id) = request.session.as_ref().unwrap().user_id {
        if user_id != id {
            return Ok(HttpResponse::unauthorized());
        }
    } else {
        return Ok(HttpResponse::unauthorized());
    }
    let new_user = serde_json::from_str::<NewUser>(&request.body[..])?;
    HttpResponse::ok()
        .json(&new_user.update(&id)?)
}

pub fn get_user(user_uuid: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    if request.session.as_ref().unwrap().user_id.is_none() {
        println!("get_user_by_uuid invalid session, session id : {:#?}", request.session.as_ref().unwrap().id);
        return Ok(HttpResponse::new(StatusCode::Unauthorized, "user should be authentified".to_string()));
    }
    println!("get_user_by_uuid valid session id : {:#?}", request.session.as_ref().unwrap().id);
    HttpResponse::ok()
        .json(&User::find(&HttpRequest::parse_uuid(user_uuid)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hash_password() {
        let mut user = User {
            uuid: Some(String::from("uuid")),
            email: String::from("email"),
            first_name: String::from("first_name"),
            last_name: String::from("last_name"),
            handle: String::from("@handle"),
            password: String::from("password"),
        };
        user.hash_password();
        assert_ne!(user.password, String::from("password"));
    }
}
