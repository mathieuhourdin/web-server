use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    interaction::model::NewInteraction,
    resource::{
        entity_type::EntityType, maturing_state::MaturingState, resource_type::ResourceType,
        NewResource,
    },
    session::Session,
};
use crate::entities_v2::trace::model::{NewTrace, TraceType};
use crate::entities_v2::lens::NewLens;
use crate::pagination::PaginationParams;
use crate::schema::{interactions, resources, users};
use argon2::Config;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
    http::StatusCode as AxumStatusCode,
    response::IntoResponse,
};
use chrono::NaiveDateTime;
use chrono::Utc;
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
    pub high_level_projects_definition: Option<String>,
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
    pub high_level_projects_definition: Option<String>,
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
            high_level_projects_definition: user.high_level_projects_definition.clone(),
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
    pub high_level_projects_definition: Option<String>,
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
            high_level_projects_definition: user.high_level_projects_definition.clone(),
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
    pub high_level_projects_definition: Option<String>,
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

pub fn ensure_user_has_meta_journal(user_id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let has_meta_journal = diesel::select(diesel::dsl::exists(
        interactions::table
            .inner_join(resources::table)
            .filter(interactions::interaction_user_id.eq(user_id))
            .filter(interactions::interaction_type.eq("outp"))
            .filter(resources::resource_type.eq(ResourceType::MetaJournal)),
    ))
    .get_result::<bool>(&mut conn)?;

    if has_meta_journal {
        return Ok(());
    }

    let meta_journal = NewResource {
        title: "Meta Journal".to_string(),
        subtitle: "".to_string(),
        content: Some("".to_string()),
        external_content_url: None,
        comment: None,
        image_url: None,
        resource_type: Some(ResourceType::MetaJournal),
        maturing_state: Some(MaturingState::Draft),
        publishing_state: Some("drft".to_string()),
        category_id: None,
        is_external: Some(false),
        entity_type: Some(EntityType::Journal),
        resource_subtype: None,
    }
    .create(pool)?;

    let mut meta_journal_interaction = NewInteraction::new(user_id, meta_journal.id);
    meta_journal_interaction.interaction_type = Some("outp".to_string());
    meta_journal_interaction.create(pool)?;

    Ok(())
}

pub fn ensure_user_has_autoplay_lens(user_id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let has_autoplay_lens = diesel::select(diesel::dsl::exists(
        interactions::table
            .inner_join(resources::table)
            .filter(interactions::interaction_user_id.eq(user_id))
            .filter(interactions::interaction_type.eq("outp"))
            .filter(resources::resource_type.eq(ResourceType::Lens))
            .filter(resources::is_external.eq(true)),
    ))
    .get_result::<bool>(&mut conn)?;

    if has_autoplay_lens {
        return Ok(());
    }

    ensure_user_has_meta_journal(user_id, pool)?;
    let placeholder_target_id = find_latest_meta_journal_id_for_user(user_id, pool)?.ok_or_else(|| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Meta journal not found after ensure".to_string(),
        )
    })?;

    let autoplay_lens = NewLens {
        name: "Autoplay Lens".to_string(),
        description: "".to_string(),
        processing_state: MaturingState::Draft,
        fork_landscape_id: None,
        target_trace_id: placeholder_target_id,
        current_state_date: Utc::now().naive_utc(),
        current_landscape_id: None,
        model_version: "".to_string(),
        autoplay: true,
        is_primary: false,
        user_id,
    };
    autoplay_lens.create(pool)?;
    Ok(())
}

fn find_latest_meta_journal_id_for_user(
    user_id: Uuid,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let journal_id = interactions::table
        .inner_join(resources::table)
        .filter(interactions::interaction_user_id.eq(user_id))
        .filter(interactions::interaction_type.eq("outp"))
        .filter(resources::resource_type.eq(ResourceType::MetaJournal))
        .order(interactions::created_at.desc())
        .select(resources::id)
        .first::<Uuid>(&mut conn)
        .optional()?;

    Ok(journal_id)
}

fn create_bio_trace_for_user(
    user_id: Uuid,
    biography: String,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    ensure_user_has_meta_journal(user_id, pool)?;
    let journal_id = find_latest_meta_journal_id_for_user(user_id, pool)?.ok_or_else(|| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Meta journal not found after ensure".to_string(),
        )
    })?;

    let mut new_trace = NewTrace::new(
        "Biography Update".to_string(),
        "".to_string(),
        biography,
        None,
        user_id,
        journal_id,
    );
    new_trace.trace_type = TraceType::BioTrace;
    new_trace.create(pool)?;
    Ok(())
}

fn create_high_level_projects_definition_trace_for_user(
    user_id: Uuid,
    high_level_projects_definition: String,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    ensure_user_has_meta_journal(user_id, pool)?;
    let journal_id = find_latest_meta_journal_id_for_user(user_id, pool)?.ok_or_else(|| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Meta journal not found after ensure".to_string(),
        )
    })?;

    let mut new_trace = NewTrace::new(
        "High Level Projects Definition Update".to_string(),
        "".to_string(),
        high_level_projects_definition,
        None,
        user_id,
        journal_id,
    );
    new_trace.trace_type = TraceType::HighLevelProjectsDefinition;
    new_trace.create(pool)?;
    Ok(())
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
    if let Err(err) = ensure_user_has_meta_journal(created_user.id, &pool) {
        if let Ok(mut conn) = pool.get() {
            let _ = diesel::delete(users::table.filter(users::id.eq(created_user.id)))
                .execute(&mut conn);
        }
        return Err(err);
    }
    if let Err(err) = ensure_user_has_autoplay_lens(created_user.id, &pool) {
        if let Ok(mut conn) = pool.get() {
            let _ = diesel::delete(users::table.filter(users::id.eq(created_user.id)))
                .execute(&mut conn);
        }
        return Err(err);
    }
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
    let new_biography = payload.biography.clone();
    let biography_changed = new_biography
        .as_ref()
        .map(|bio| existing_user.biography.as_ref() != Some(bio))
        .unwrap_or(false);
    let new_hlp_definition = payload.high_level_projects_definition.clone();
    let hlp_definition_changed = new_hlp_definition
        .as_ref()
        .map(|definition| existing_user.high_level_projects_definition.as_ref() != Some(definition))
        .unwrap_or(false);

    if &session_user_id != &id && existing_user.is_platform_user {
        return Err(PpdcError::unauthorized());
    }
    let updated_user = payload.update(&id, &pool)?;

    if biography_changed {
        if let Some(biography) = new_biography {
            create_bio_trace_for_user(id, biography, &pool)?;
        }
    }
    if hlp_definition_changed {
        if let Some(high_level_projects_definition) = new_hlp_definition {
            create_high_level_projects_definition_trace_for_user(
                id,
                high_level_projects_definition,
                &pool,
            )?;
        }
    }

    Ok(Json(updated_user))
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
            high_level_projects_definition: None,
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
