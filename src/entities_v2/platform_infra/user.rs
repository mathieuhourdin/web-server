use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    journal::{Journal, JournalType, NewJournalDto},
    lens::{LensProcessingState, NewLens},
    session::Session,
    trace::{NewTrace, TraceType},
};
use crate::pagination::PaginationParams;
use crate::schema::{journals, lenses, user_roles, users};
use argon2::Config;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
    http::StatusCode as AxumStatusCode,
    response::IntoResponse,
};
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc, Weekday};
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::prelude::*;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_query;
use diesel::sql_types::{BigInt, Date, Float, Nullable, SmallInt, Text, Uuid as SqlUuid};
use diesel::{AsExpression, FromSqlRow};
use rand::Rng;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub enum JournalTheme {
    Classic,
    White,
    Flowers,
    Dark,
}

impl JournalTheme {
    pub fn to_code(self) -> &'static str {
        match self {
            JournalTheme::Classic => "classic",
            JournalTheme::White => "white",
            JournalTheme::Flowers => "flowers",
            JournalTheme::Dark => "dark",
        }
    }

    pub fn from_code(code: &str) -> Result<Self, PpdcError> {
        match code {
            "classic" | "Classic" => Ok(JournalTheme::Classic),
            // Backward compatibility with previous default naming.
            "default" | "Default" => Ok(JournalTheme::Classic),
            "white" | "White" => Ok(JournalTheme::White),
            "flowers" | "Flowers" => Ok(JournalTheme::Flowers),
            "dark" | "Dark" => Ok(JournalTheme::Dark),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid journal_theme: {}", code),
            )),
        }
    }

    pub fn to_api_value(self) -> &'static str {
        match self {
            JournalTheme::Classic => "classic",
            JournalTheme::White => "white",
            JournalTheme::Flowers => "flowers",
            JournalTheme::Dark => "dark",
        }
    }
}

impl Serialize for JournalTheme {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_api_value())
    }
}

impl<'de> Deserialize<'de> for JournalTheme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        JournalTheme::from_code(&value).map_err(|_| de::Error::custom("unknown journal_theme"))
    }
}

impl ToSql<Text, Pg> for JournalTheme {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_code(), out)
    }
}

impl FromSql<Text, Pg> for JournalTheme {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        JournalTheme::from_code(value.as_str())
            .map_err(|_| "invalid journal_theme value in database".into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::SmallInt)]
pub enum WeekAnalysisWeekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl WeekAnalysisWeekday {
    pub fn to_db_value(self) -> i16 {
        match self {
            WeekAnalysisWeekday::Monday => 1,
            WeekAnalysisWeekday::Tuesday => 2,
            WeekAnalysisWeekday::Wednesday => 3,
            WeekAnalysisWeekday::Thursday => 4,
            WeekAnalysisWeekday::Friday => 5,
            WeekAnalysisWeekday::Saturday => 6,
            WeekAnalysisWeekday::Sunday => 7,
        }
    }

    pub fn from_db_value(value: i16) -> Result<Self, PpdcError> {
        match value {
            1 => Ok(WeekAnalysisWeekday::Monday),
            2 => Ok(WeekAnalysisWeekday::Tuesday),
            3 => Ok(WeekAnalysisWeekday::Wednesday),
            4 => Ok(WeekAnalysisWeekday::Thursday),
            5 => Ok(WeekAnalysisWeekday::Friday),
            6 => Ok(WeekAnalysisWeekday::Saturday),
            7 => Ok(WeekAnalysisWeekday::Sunday),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid week_analysis_weekday: {}", value),
            )),
        }
    }

    pub fn from_code(code: &str) -> Result<Self, PpdcError> {
        match code {
            "1" => Ok(WeekAnalysisWeekday::Monday),
            "2" => Ok(WeekAnalysisWeekday::Tuesday),
            "3" => Ok(WeekAnalysisWeekday::Wednesday),
            "4" => Ok(WeekAnalysisWeekday::Thursday),
            "5" => Ok(WeekAnalysisWeekday::Friday),
            "6" => Ok(WeekAnalysisWeekday::Saturday),
            "7" => Ok(WeekAnalysisWeekday::Sunday),
            "MONDAY" | "Monday" | "monday" => Ok(WeekAnalysisWeekday::Monday),
            "TUESDAY" | "Tuesday" | "tuesday" => Ok(WeekAnalysisWeekday::Tuesday),
            "WEDNESDAY" | "Wednesday" | "wednesday" => Ok(WeekAnalysisWeekday::Wednesday),
            "THURSDAY" | "Thursday" | "thursday" => Ok(WeekAnalysisWeekday::Thursday),
            "FRIDAY" | "Friday" | "friday" => Ok(WeekAnalysisWeekday::Friday),
            "SATURDAY" | "Saturday" | "saturday" => Ok(WeekAnalysisWeekday::Saturday),
            "SUNDAY" | "Sunday" | "sunday" => Ok(WeekAnalysisWeekday::Sunday),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid week_analysis_weekday: {}", code),
            )),
        }
    }

    pub fn to_api_value(self) -> &'static str {
        match self {
            WeekAnalysisWeekday::Monday => "monday",
            WeekAnalysisWeekday::Tuesday => "tuesday",
            WeekAnalysisWeekday::Wednesday => "wednesday",
            WeekAnalysisWeekday::Thursday => "thursday",
            WeekAnalysisWeekday::Friday => "friday",
            WeekAnalysisWeekday::Saturday => "saturday",
            WeekAnalysisWeekday::Sunday => "sunday",
        }
    }

    pub fn to_chrono_weekday(self) -> Weekday {
        match self {
            WeekAnalysisWeekday::Monday => Weekday::Mon,
            WeekAnalysisWeekday::Tuesday => Weekday::Tue,
            WeekAnalysisWeekday::Wednesday => Weekday::Wed,
            WeekAnalysisWeekday::Thursday => Weekday::Thu,
            WeekAnalysisWeekday::Friday => Weekday::Fri,
            WeekAnalysisWeekday::Saturday => Weekday::Sat,
            WeekAnalysisWeekday::Sunday => Weekday::Sun,
        }
    }
}

impl Serialize for WeekAnalysisWeekday {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_api_value())
    }
}

impl<'de> Deserialize<'de> for WeekAnalysisWeekday {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        WeekAnalysisWeekday::from_code(&value)
            .map_err(|_| de::Error::custom("unknown week_analysis_weekday"))
    }
}

impl ToSql<SmallInt, Pg> for WeekAnalysisWeekday {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        let value = self.to_db_value();
        <i16 as ToSql<SmallInt, Pg>>::to_sql(&value, &mut out.reborrow())
    }
}

impl FromSql<SmallInt, Pg> for WeekAnalysisWeekday {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let value = <i16 as FromSql<SmallInt, Pg>>::from_sql(bytes)?;
        WeekAnalysisWeekday::from_db_value(value)
            .map_err(|_| "invalid week_analysis_weekday value in database".into())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Member,
    Admin,
    Mentor,
}

impl UserRole {
    pub fn to_db(self) -> &'static str {
        match self {
            UserRole::Member => "MEMBER",
            UserRole::Admin => "ADMIN",
            UserRole::Mentor => "MENTOR",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, PpdcError> {
        match value {
            "MEMBER" | "member" => Ok(UserRole::Member),
            "ADMIN" | "admin" => Ok(UserRole::Admin),
            "MENTOR" | "mentor" => Ok(UserRole::Mentor),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid user role: {}", value),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub enum UserPrincipalType {
    Human,
    Service,
}

impl UserPrincipalType {
    pub fn to_db(self) -> &'static str {
        match self {
            UserPrincipalType::Human => "HUMAN",
            UserPrincipalType::Service => "SERVICE",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, PpdcError> {
        match value {
            "HUMAN" | "human" => Ok(UserPrincipalType::Human),
            "SERVICE" | "service" => Ok(UserPrincipalType::Service),
            _ => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid user principal_type: {}", value),
            )),
        }
    }

    pub fn to_api_value(self) -> &'static str {
        match self {
            UserPrincipalType::Human => "human",
            UserPrincipalType::Service => "service",
        }
    }
}

impl Serialize for UserPrincipalType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_api_value())
    }
}

impl<'de> Deserialize<'de> for UserPrincipalType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        UserPrincipalType::from_db(&value)
            .map_err(|_| de::Error::custom("unknown principal_type"))
    }
}

impl ToSql<Text, Pg> for UserPrincipalType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_db(), out)
    }
}

impl FromSql<Text, Pg> for UserPrincipalType {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let value = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        UserPrincipalType::from_db(value.as_str())
            .map_err(|_| "invalid principal_type value in database".into())
    }
}

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, Insertable, Identifiable, Debug)]
#[diesel(table_name = crate::schema::user_roles)]
pub struct UserRoleAssignment {
    pub id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Deserialize, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::users)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub principal_type: UserPrincipalType,
    pub mentor_id: Option<Uuid>,
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
    pub high_level_projects_definition: Option<String>,
    pub journal_theme: JournalTheme,
    pub current_lens_id: Option<Uuid>,
    pub week_analysis_weekday: WeekAnalysisWeekday,
    pub timezone: String,
    pub context_anchor_at: Option<NaiveDateTime>,
    pub welcome_message: Option<String>,
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
    pub principal_type: UserPrincipalType,
    pub mentor_id: Option<Uuid>,
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
    pub journal_theme: JournalTheme,
    pub current_lens_id: Option<Uuid>,
    pub week_analysis_weekday: WeekAnalysisWeekday,
    pub timezone: String,
    pub context_anchor_at: Option<NaiveDateTime>,
    pub welcome_message: Option<String>,
    pub display_name: String,
}

impl From<&User> for UserPseudonymizedAuthentifiedResponse {
    fn from(user: &User) -> Self {
        UserPseudonymizedAuthentifiedResponse {
            id: user.id.clone(),
            email: user.email.clone(),
            principal_type: user.principal_type,
            mentor_id: user.mentor_id,
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
            journal_theme: user.journal_theme,
            current_lens_id: user.current_lens_id,
            week_analysis_weekday: user.week_analysis_weekday,
            timezone: user.timezone.clone(),
            context_anchor_at: user.context_anchor_at,
            welcome_message: user.welcome_message.clone(),
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
    pub principal_type: UserPrincipalType,
    pub mentor_id: Option<Uuid>,
    pub handle: String,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
    pub profile_picture_url: Option<String>,
    pub is_platform_user: bool,
    pub biography: Option<String>,
    pub high_level_projects_definition: Option<String>,
    pub pseudonymized: bool,
    pub journal_theme: JournalTheme,
    pub current_lens_id: Option<Uuid>,
    pub week_analysis_weekday: WeekAnalysisWeekday,
    pub timezone: String,
    pub context_anchor_at: Option<NaiveDateTime>,
    pub welcome_message: Option<String>,
    pub display_name: String,
}

impl From<&User> for UserPseudonymizedResponse {
    fn from(user: &User) -> Self {
        UserPseudonymizedResponse {
            id: user.id.clone(),
            principal_type: user.principal_type,
            mentor_id: user.mentor_id,
            handle: user.handle.clone(),
            created_at: user.created_at.clone(),
            updated_at: user.updated_at.clone(),
            profile_picture_url: user.profile_picture_url.clone(),
            is_platform_user: user.is_platform_user.clone(),
            biography: user.biography.clone(),
            high_level_projects_definition: user.high_level_projects_definition.clone(),
            pseudonymized: user.pseudonymized.clone(),
            journal_theme: user.journal_theme,
            current_lens_id: user.current_lens_id,
            week_analysis_weekday: user.week_analysis_weekday,
            timezone: user.timezone.clone(),
            context_anchor_at: user.context_anchor_at,
            welcome_message: user.welcome_message.clone(),
            display_name: if user.pseudonymized {
                user.pseudonym.clone()
            } else {
                user.first_name.clone() + " " + user.last_name.as_str()
            },
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct UserSearchParams {
    pub q: String,
    pub limit: Option<i64>,
}

#[derive(QueryableByName)]
struct UserSearchRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    handle: String,
    #[diesel(sql_type = Nullable<Text>)]
    first_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    last_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    profile_picture_url: Option<String>,
    #[diesel(sql_type = Text)]
    principal_type: String,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    pseudonymized: bool,
    #[diesel(sql_type = Text)]
    pseudonym: String,
}

#[derive(Serialize)]
pub struct UserSearchResult {
    pub id: Uuid,
    pub handle: String,
    pub principal_type: UserPrincipalType,
    pub profile_picture_url: Option<String>,
    pub display_name: String,
}

impl From<UserSearchRow> for UserSearchResult {
    fn from(row: UserSearchRow) -> Self {
        let principal_type = UserPrincipalType::from_db(&row.principal_type)
            .unwrap_or(UserPrincipalType::Human);
        let display_name = if row.pseudonymized {
            row.pseudonym
        } else {
            match (row.first_name, row.last_name) {
                (Some(first_name), Some(last_name))
                    if !first_name.trim().is_empty() || !last_name.trim().is_empty() =>
                {
                    format!("{} {}", first_name, last_name).trim().to_string()
                }
                (Some(first_name), None) if !first_name.trim().is_empty() => first_name,
                (None, Some(last_name)) if !last_name.trim().is_empty() => last_name,
                _ => row.handle.clone(),
            }
        };

        UserSearchResult {
            id: row.id,
            handle: row.handle,
            principal_type,
            profile_picture_url: row.profile_picture_url,
            display_name,
        }
    }
}

impl From<&User> for UserSearchResult {
    fn from(user: &User) -> Self {
        UserSearchResult {
            id: user.id,
            handle: user.handle.clone(),
            principal_type: user.principal_type,
            profile_picture_url: user.profile_picture_url.clone(),
            display_name: user.display_name(),
        }
    }
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::users)]
pub struct NewUser {
    pub email: String,
    pub principal_type: Option<UserPrincipalType>,
    pub mentor_id: Option<Uuid>,
    pub first_name: String,
    pub last_name: String,
    pub handle: String,
    pub password: Option<String>,
    pub profile_picture_url: Option<String>,
    pub is_platform_user: Option<bool>,
    pub biography: Option<String>,
    pub pseudonym: Option<String>,
    pub pseudonymized: Option<bool>,
    pub high_level_projects_definition: Option<String>,
    pub journal_theme: Option<JournalTheme>,
    pub current_lens_id: Option<Uuid>,
    pub week_analysis_weekday: Option<WeekAnalysisWeekday>,
    pub timezone: Option<String>,
    pub context_anchor_at: Option<NaiveDateTime>,
    pub welcome_message: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct NewServiceUserDto {
    pub first_name: String,
    pub last_name: String,
    pub biography: Option<String>,
    pub profile_picture_url: Option<String>,
    pub welcome_message: Option<String>,
}

impl NewServiceUserDto {
    fn to_new_user(self) -> NewUser {
        let generated_handle = format!("@{}-{}", self.first_name.trim(), self.last_name.trim());
        let normalized_handle = generated_handle.trim().to_string();
        let slug = normalized_handle
            .trim_start_matches('@')
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>();
        let email_slug = if slug.is_empty() {
            "service-user".to_string()
        } else {
            slug
        };
        NewUser {
            email: format!("{}@service.internal", email_slug),
            principal_type: Some(UserPrincipalType::Service),
            mentor_id: None,
            first_name: self.first_name,
            last_name: self.last_name,
            handle: normalized_handle,
            password: None,
            profile_picture_url: self.profile_picture_url,
            is_platform_user: Some(false),
            biography: self.biography,
            pseudonym: None,
            pseudonymized: Some(false),
            high_level_projects_definition: None,
            journal_theme: None,
            current_lens_id: None,
            week_analysis_weekday: None,
            timezone: Some("UTC".to_string()),
            context_anchor_at: None,
            welcome_message: self.welcome_message,
        }
    }

    fn to_service_user_update(self, existing_user: &User) -> NewUser {
        let generated_handle = format!("@{}-{}", self.first_name.trim(), self.last_name.trim());
        let normalized_handle = generated_handle.trim().to_string();

        NewUser {
            email: existing_user.email.clone(),
            principal_type: Some(UserPrincipalType::Service),
            mentor_id: existing_user.mentor_id,
            first_name: self.first_name,
            last_name: self.last_name,
            handle: normalized_handle,
            password: Some(existing_user.password.clone()),
            profile_picture_url: self.profile_picture_url,
            is_platform_user: Some(existing_user.is_platform_user),
            biography: self.biography,
            pseudonym: Some(existing_user.pseudonym.clone()),
            pseudonymized: Some(existing_user.pseudonymized),
            high_level_projects_definition: existing_user.high_level_projects_definition.clone(),
            journal_theme: Some(existing_user.journal_theme),
            current_lens_id: existing_user.current_lens_id,
            week_analysis_weekday: Some(existing_user.week_analysis_weekday),
            timezone: Some(existing_user.timezone.clone()),
            context_anchor_at: existing_user.context_anchor_at,
            welcome_message: self.welcome_message,
        }
    }
}

impl NewUser {
    pub fn create(self, pool: &DbPool) -> Result<User, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let mut payload = self;
        if payload.principal_type.is_none() {
            payload.principal_type = Some(UserPrincipalType::Human);
        }

        let user = conn.transaction::<User, diesel::result::Error, _>(|conn| {
            let user = diesel::insert_into(users::table)
                .values(&payload)
                .returning(User::as_returning())
                .get_result(conn)?;

            diesel::insert_into(user_roles::table)
                .values((
                    user_roles::user_id.eq(user.id),
                    user_roles::role.eq(UserRole::Member.to_db()),
                ))
                .on_conflict((user_roles::user_id, user_roles::role))
                .do_nothing()
                .execute(conn)?;

            Ok(user)
        })?;
        Ok(user)
    }

    pub fn update(self, id: &Uuid, pool: &DbPool) -> Result<User, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let result = diesel::update(users::table)
            .filter(users::id.eq(id))
            .set(&self)
            .returning(User::as_returning())
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

        let user = users::table
            .filter(users::id.eq(id))
            .select(User::as_select())
            .first(&mut conn)?;
        Ok(user)
    }

    pub fn find_by_username(email: &String, pool: &DbPool) -> Result<User, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let user = users::table
            .filter(users::email.eq(email))
            .select(User::as_select())
            .first(&mut conn)?;
        Ok(user)
    }

    pub fn find_many(ids: &[Uuid], pool: &DbPool) -> Result<Vec<User>, PpdcError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let users = users::table
            .filter(users::id.eq_any(ids))
            .select(User::as_select())
            .load(&mut conn)?;
        Ok(users)
    }

    pub fn display_name(&self) -> String {
        if self.pseudonymized {
            self.pseudonym.clone()
        } else {
            format!("{} {}", self.first_name, self.last_name)
        }
    }

    pub fn list_roles(&self, pool: &DbPool) -> Result<Vec<UserRole>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let role_values = user_roles::table
            .filter(user_roles::user_id.eq(self.id))
            .select(user_roles::role)
            .load::<String>(&mut conn)?;

        role_values
            .into_iter()
            .map(|role| UserRole::from_db(&role))
            .collect()
    }

    pub fn has_role(&self, role: UserRole, pool: &DbPool) -> Result<bool, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let has_role = diesel::select(diesel::dsl::exists(
            user_roles::table
                .filter(user_roles::user_id.eq(self.id))
                .filter(user_roles::role.eq(role.to_db())),
        ))
        .get_result::<bool>(&mut conn)?;
        Ok(has_role)
    }

    pub fn add_role(&self, role: UserRole, pool: &DbPool) -> Result<(), PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        diesel::insert_into(user_roles::table)
            .values((
                user_roles::user_id.eq(self.id),
                user_roles::role.eq(role.to_db()),
            ))
            .on_conflict((user_roles::user_id, user_roles::role))
            .do_nothing()
            .execute(&mut conn)?;
        Ok(())
    }
}

pub fn ensure_user_has_meta_journal(user_id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let has_meta_journal = diesel::select(diesel::dsl::exists(
        journals::table
            .filter(journals::user_id.eq(user_id))
            .filter(journals::journal_type.eq(JournalType::MetaJournal.to_db())),
    ))
    .get_result::<bool>(&mut conn)?;

    if has_meta_journal {
        return Ok(());
    }

    Journal::create(
        NewJournalDto {
            title: "Meta Journal".to_string(),
            subtitle: Some(String::new()),
            content: Some(String::new()),
            is_encrypted: Some(false),
            journal_type: Some(JournalType::MetaJournal),
        },
        user_id,
        pool,
    )?;

    Ok(())
}

pub fn ensure_user_has_autoplay_lens(user_id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let has_autoplay_lens = diesel::select(diesel::dsl::exists(
        lenses::table
            .filter(lenses::user_id.eq(user_id))
            .filter(lenses::autoplay.eq(true)),
    ))
    .get_result::<bool>(&mut conn)?;

    if has_autoplay_lens {
        return Ok(());
    }

    let autoplay_lens = NewLens {
        processing_state: LensProcessingState::InSync,
        fork_landscape_id: None,
        target_trace_id: None,
        current_landscape_id: None,
        autoplay: true,
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

    let journal_id = journals::table
        .filter(journals::user_id.eq(user_id))
        .filter(journals::journal_type.eq(JournalType::MetaJournal.to_db()))
        .order(journals::created_at.desc())
        .select(journals::id)
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
        Utc::now().naive_utc(),
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
        Utc::now().naive_utc(),
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

#[derive(QueryableByName)]
struct UserIdRow {
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct UserDailyActivityRow {
    #[diesel(sql_type = Date)]
    day: NaiveDate,
    #[diesel(sql_type = BigInt)]
    trace_count: i64,
    #[diesel(sql_type = BigInt)]
    element_count: i64,
    #[diesel(sql_type = BigInt)]
    landmark_count: i64,
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

#[derive(Serialize)]
pub struct AdminUserDailyActivity {
    pub day: NaiveDate,
    pub written_traces: i64,
    pub created_elements: i64,
    pub created_landmarks: i64,
}

#[derive(Serialize)]
pub struct AdminUserRecentActivity {
    pub id: Uuid,
    pub hlp_landmarks_count: i64,
    pub heatmap: Vec<AdminUserDailyActivity>,
}

fn ensure_admin_session_user(session: &Session, pool: &DbPool) -> Result<User, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let session_user = User::find(&session_user_id, pool)?;
    if !session_user.has_role(UserRole::Admin, pool)? {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "Admin role required".to_string(),
        ));
    }
    Ok(session_user)
}

#[debug_handler]
pub async fn get_admin_recent_user_activity_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<AdminUserRecentActivity>>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;

    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let user_rows = sql_query(
        r#"
        SELECT user_id
        FROM traces
        GROUP BY user_id
        ORDER BY MAX(COALESCE(interaction_date, created_at)) DESC
        LIMIT 10
        "#,
    )
    .load::<UserIdRow>(&mut conn)?;

    let to = Utc::now().date_naive();
    let from = to - Duration::days(29);

    let mut payload = Vec::with_capacity(user_rows.len());

    for row in user_rows {
        let hlp_landmarks_count = sql_query(
            r#"
            SELECT COUNT(*)::bigint AS value
            FROM landmarks
            WHERE user_id = $1
              AND landmark_type = 'HIGH_LEVEL_PROJECT'
            "#,
        )
        .bind::<SqlUuid, _>(row.user_id)
        .get_result::<CountRow>(&mut conn)?
        .value;

        let heatmap_rows = sql_query(
            r#"
            WITH days AS (
                SELECT generate_series($2::date, $3::date, interval '1 day')::date AS day
            )
            SELECT
                d.day AS day,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM traces t
                    WHERE t.user_id = $1
                      AND COALESCE(t.interaction_date, t.created_at)::date = d.day
                ), 0)::bigint AS trace_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM elements e
                    WHERE e.user_id = $1
                      AND e.created_at::date = d.day
                ), 0)::bigint AS element_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM landmarks l
                    WHERE l.user_id = $1
                      AND l.created_at::date = d.day
                ), 0)::bigint AS landmark_count
            FROM days d
            ORDER BY d.day
            "#,
        )
        .bind::<SqlUuid, _>(row.user_id)
        .bind::<Date, _>(from)
        .bind::<Date, _>(to)
        .load::<UserDailyActivityRow>(&mut conn)?;

        let heatmap = heatmap_rows
            .into_iter()
            .map(|r| AdminUserDailyActivity {
                day: r.day,
                written_traces: r.trace_count,
                created_elements: r.element_count,
                created_landmarks: r.landmark_count,
            })
            .collect::<Vec<_>>();

        payload.push(AdminUserRecentActivity {
            id: row.user_id,
            hlp_landmarks_count,
            heatmap,
        });
    }

    Ok(Json(payload))
}

#[debug_handler]
pub async fn get_admin_service_users_route(
    Query(params): Query<PaginationParams>,
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<User>>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let users = users::table
        .filter(users::principal_type.eq(UserPrincipalType::Service))
        .offset(params.offset())
        .limit(params.limit())
        .order(users::created_at.desc())
        .select(User::as_select())
        .load::<User>(&mut conn)?;

    Ok(Json(users))
}

#[debug_handler]
pub async fn post_admin_service_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewServiceUserDto>,
) -> Result<Json<User>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let mut payload = payload.to_new_user();
    payload.hash_password()?;

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
pub async fn get_admin_service_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let user = User::find(&id, &pool)?;
    if user.principal_type != UserPrincipalType::Service {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "Service user not found".to_string(),
        ));
    }
    Ok(Json(user))
}

#[debug_handler]
pub async fn put_admin_service_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewServiceUserDto>,
) -> Result<Json<User>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let existing_user = User::find(&id, &pool)?;
    if existing_user.principal_type != UserPrincipalType::Service {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "Service user not found".to_string(),
        ));
    }

    let payload = payload.to_service_user_update(&existing_user);
    let updated_user = payload.update(&id, &pool)?;
    Ok(Json(updated_user))
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
        .select(User::as_select())
        .load::<User>(&mut conn)?
        .iter()
        .map(UserPseudonymizedResponse::from)
        .collect();
    Ok(Json(results))
}

#[debug_handler]
pub async fn get_user_search_route(
    Query(params): Query<UserSearchParams>,
    Extension(pool): Extension<DbPool>,
) -> Result<Json<Vec<UserSearchResult>>, PpdcError> {
    let query = params.q.trim();
    if query.is_empty() {
        return Ok(Json(vec![]));
    }

    let limit = params.limit.unwrap_or(20).clamp(1, 50);
    let contains_query = format!("%{}%", query);
    let prefix_query = format!("{}%", query);

    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let rows = sql_query(
        r#"
        SELECT
            id,
            handle,
            NULLIF(first_name, '') AS first_name,
            NULLIF(last_name, '') AS last_name,
            profile_picture_url,
            principal_type,
            pseudonymized,
            pseudonym
        FROM users
        WHERE principal_type = 'HUMAN'
          AND (
            handle ILIKE $1
            OR first_name ILIKE $1
            OR last_name ILIKE $1
            OR CONCAT_WS(' ', first_name, last_name) ILIKE $1
          )
        ORDER BY
          CASE
            WHEN LOWER(handle) = LOWER($2) THEN 0
            WHEN handle ILIKE $3 THEN 1
            WHEN first_name ILIKE $3 OR last_name ILIKE $3 THEN 2
            WHEN CONCAT_WS(' ', first_name, last_name) ILIKE $3 THEN 3
            ELSE 4
          END,
          updated_at DESC NULLS LAST,
          created_at DESC
        LIMIT $4
        "#,
    )
    .bind::<Text, _>(contains_query)
    .bind::<Text, _>(query.to_string())
    .bind::<Text, _>(prefix_query)
    .bind::<BigInt, _>(limit)
    .load::<UserSearchRow>(&mut conn)?;

    Ok(Json(rows.into_iter().map(UserSearchResult::from).collect()))
}

#[debug_handler]
pub async fn get_mentors_route(
    Query(params): Query<PaginationParams>,
    Extension(pool): Extension<DbPool>,
) -> Result<Json<Vec<UserPseudonymizedResponse>>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let results: Vec<UserPseudonymizedResponse> = users::table
        .filter(users::principal_type.eq(UserPrincipalType::Service))
        .offset(params.offset())
        .limit(params.limit())
        .order(users::created_at.desc())
        .select(User::as_select())
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
            principal_type: None,
            mentor_id: None,
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
            journal_theme: None,
            current_lens_id: None,
            week_analysis_weekday: None,
            timezone: None,
            context_anchor_at: None,
            welcome_message: None,
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
