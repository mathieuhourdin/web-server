use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    session::Session,
};
use crate::pagination::PaginationParams;
use crate::schema::users;
use argon2::Config;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
    http::StatusCode as AxumStatusCode,
    response::IntoResponse,
};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Float, Text, Uuid as SqlUuid};
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub profile_picture_url: Option<String>,
    pub is_platform_user: bool,
    pub biography: Option<String>,
    pub pseudonym: String,
    pub pseudonymized: bool,
}

pub enum UserResponse {
    Pseudonymized(UserPseudonymizedResponse),
    PseudonymizedAuthentified(UserPseudonymizedAuthentifiedResponse),
    Full(User),
}

impl IntoResponse for UserResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            UserResponse::Pseudonymized(user) => (AxumStatusCode::OK, Json(user)).into_response(),
            UserResponse::PseudonymizedAuthentified(user) => {
                (AxumStatusCode::OK, Json(user)).into_response()
            }
            UserResponse::Full(user) => (AxumStatusCode::OK, Json(user)).into_response(),
        }
    }
}

#[derive(Serialize)]
pub struct UserPseudonymizedAuthentifiedResponse {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub handle: String,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
    pub profile_picture_url: Option<String>,
    pub is_platform_user: bool,
    pub biography: Option<String>,
    pub pseudonymized: bool,
    pub display_name: String,
}

impl From<&User> for UserPseudonymizedAuthentifiedResponse {
    fn from(user: &User) -> Self {
        UserPseudonymizedAuthentifiedResponse {
            id: user.id.clone(),
            email: user.email.clone(),
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone(),
            handle: user.handle.clone(),
            created_at: user.created_at.clone(),
            updated_at: user.updated_at.clone(),
            profile_picture_url: user.profile_picture_url.clone(),
            is_platform_user: user.is_platform_user.clone(),
            biography: user.biography.clone(),
            pseudonymized: user.pseudonymized.clone(),
            display_name: if user.pseudonymized {
                user.pseudonym.clone()
            } else {
                user.first_name.clone() + " " + user.last_name.as_str()
            },
        }
    }
}

#[derive(Serialize)]
pub struct UserPseudonymizedResponse {
    pub id: Uuid,
    pub handle: String,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
    pub profile_picture_url: Option<String>,
    pub is_platform_user: bool,
    pub biography: Option<String>,
    pub pseudonymized: bool,
    pub display_name: String,
}

impl From<&User> for UserPseudonymizedResponse {
    fn from(user: &User) -> Self {
        UserPseudonymizedResponse {
            id: user.id.clone(),
            handle: user.handle.clone(),
            created_at: user.created_at.clone(),
            updated_at: user.updated_at.clone(),
            profile_picture_url: user.profile_picture_url.clone(),
            is_platform_user: user.is_platform_user.clone(),
            biography: user.biography.clone(),
            pseudonymized: user.pseudonymized.clone(),
            display_name: if user.pseudonymized {
                user.pseudonym.clone()
            } else {
                user.first_name.clone() + " " + user.last_name.as_str()
            },
        }
    }
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::users)]
pub struct NewUser {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub handle: String,
    pub password: Option<String>,
    pub profile_picture_url: Option<String>,
    pub is_platform_user: Option<bool>,
    pub biography: Option<String>,
    pub pseudonym: Option<String>,
    pub pseudonymized: Option<bool>,
}

impl NewUser {
    pub fn create(self, pool: &DbPool) -> Result<User, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let user = diesel::insert_into(users::table)
            .values(&self)
            .get_result(&mut conn)?;
        Ok(user)
    }

    pub fn update(self, id: &Uuid, pool: &DbPool) -> Result<User, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

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
            self.password = Some(
                argon2::hash_encoded(password.as_bytes(), &salt, &config).map_err(|err| {
                    PpdcError::new(
                        500,
                        ErrorType::InternalError,
                        format!("Unable to encode password: {:#?}", err),
                    )
                })?,
            );
            Ok(())
        } else {
            if self.is_platform_user.is_none() || self.is_platform_user.unwrap() == false {
                self.password = Some(String::new());
                Ok(())
            } else {
                Err(PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!("No password to encode"),
                ))
            }
        }
    }
}

impl User {
    pub fn verify_password(&self, tested_password_bytes: &[u8]) -> Result<bool, PpdcError> {
        argon2::verify_encoded(&self.password, tested_password_bytes).map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unable to decode password: {:#?}", err),
            )
        })
    }

    pub fn find(id: &Uuid, pool: &DbPool) -> Result<User, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let user = users::table.filter(users::id.eq(id)).first(&mut conn)?;
        Ok(user)
    }

    pub fn find_by_username(email: &String, pool: &DbPool) -> Result<User, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let user = users::table
            .filter(users::email.eq(email))
            .first(&mut conn)?;
        Ok(user)
    }

    pub fn display_name(&self) -> String {
        if self.pseudonymized {
            self.pseudonym.clone()
        } else {
            format!("{} {}", self.first_name, self.last_name)
        }
    }
}

#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    first_name: String,
    #[diesel(sql_type = Text)]
    last_name: String,
    #[diesel(sql_type = Float)]
    score: f32,
}

#[derive(Debug)]
pub struct UserMatch {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub score: f32,
}

pub fn find_similar_users(
    pool: &DbPool,
    input: &str,
    limit: i64,
) -> Result<Vec<UserMatch>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let query = format!(
        "
        SELECT id, first_name, last_name,
               similarity(first_name || ' ' || last_name, $1) AS score
        FROM users
        WHERE (first_name || ' ' || last_name) % $1
        ORDER BY score DESC
        LIMIT {}
        ",
        limit
    );

    let rows: Vec<Row> = sql_query(query)
        .bind::<Text, _>(input)
        .load(&mut conn)
        .map_err(|e| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Database error: {}", e),
            )
        })?;

    Ok(rows
        .into_iter()
        .map(|r| UserMatch {
            id: r.id,
            first_name: r.first_name,
            last_name: r.last_name,
            score: r.score,
        })
        .collect())
}

#[debug_handler]
pub async fn get_users(
    Query(params): Query<PaginationParams>,
    Extension(pool): Extension<DbPool>,
) -> Result<Json<Vec<UserPseudonymizedResponse>>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let results: Vec<UserPseudonymizedResponse> = users::table
        .into_boxed()
        .offset(params.offset())
        .limit(params.limit())
        .load::<User>(&mut conn)?
        .iter()
        .map(UserPseudonymizedResponse::from)
        .collect();
    Ok(Json(results))
}

#[debug_handler]
pub async fn post_user(
    Extension(pool): Extension<DbPool>,
    Json(mut payload): Json<NewUser>,
) -> Result<Json<User>, PpdcError> {
    payload.hash_password().unwrap();
    let created_user = payload.create(&pool)?;
    Ok(Json(created_user))
}

#[debug_handler]
pub async fn put_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewUser>,
) -> Result<Json<User>, PpdcError> {
    let session_user_id = session.user_id.unwrap();
    let existing_user = User::find(&id, &pool)?;

    if &session_user_id != &id && existing_user.is_platform_user {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(payload.update(&id, &pool)?))
}

#[debug_handler]
pub async fn get_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, PpdcError> {
    let user = User::find(&id, &pool)?;
    if !user.is_platform_user || session.user_id.unwrap() == id {
        let user_response = UserPseudonymizedAuthentifiedResponse::from(&user);
        Ok(UserResponse::PseudonymizedAuthentified(user_response))
    } else {
        let user_response = UserPseudonymizedResponse::from(&user);
        Ok(UserResponse::Pseudonymized(user_response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password() {
        let mut user = NewUser {
            email: String::from("email"),
            first_name: String::from("first_name"),
            last_name: String::from("last_name"),
            handle: String::from("@handle"),
            password: Some(String::from("password")),
            profile_picture_url: None,
            is_platform_user: None,
            biography: None,
            pseudonym: None,
            pseudonymized: Some(false),
        };
        user.hash_password().unwrap();
        assert_ne!(user.password, Some(String::from("password")));
    }

    #[test]
    fn test_find_similar_users() {
        use crate::environment::get_database_url;
        let database_url = get_database_url();
        let manager = diesel::r2d2::ConnectionManager::new(database_url);
        let pool = DbPool::new(manager).expect("Failed to create connection pool");

        let results = find_similar_users(&pool, "Hourdin Matthieu", 3).unwrap();
        assert!(!results.is_empty());
        println!("Résultats: {:?}", results);
    }
}
