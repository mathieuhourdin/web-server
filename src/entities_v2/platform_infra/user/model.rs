use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    journal::{Journal, JournalType, NewJournalDto},
    lens::{LensProcessingState, NewLens},
    trace::{NewTrace, TraceType},
};
use crate::pagination::PaginationParams;
use crate::schema::{journals, lenses, user_roles, users};
use argon2::Config;
use axum::{extract::Json, http::StatusCode as AxumStatusCode, response::IntoResponse};
use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use diesel::sql_query;
use diesel::sql_types::{Float, Nullable, Text, Uuid as SqlUuid};
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::{
    EmailNotificationMode, HomeFocusView, JournalTheme, UserPrincipalType, UserRole,
    WeekAnalysisWeekday,
};

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
    pub profile_asset_id: Option<Uuid>,
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
    pub home_focus_view: HomeFocusView,
    pub shared_journal_activity_email_mode: EmailNotificationMode,
    pub received_message_email_mode: EmailNotificationMode,
    pub mentor_feedback_email_enabled: bool,
}

pub enum UserResponse {
    Public(UserPublicResponse),
    PseudonymizedAuthentified(UserPseudonymizedAuthentifiedResponse),
    Full(User),
}

impl IntoResponse for UserResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            UserResponse::Public(user) => (AxumStatusCode::OK, Json(user)).into_response(),
            UserResponse::PseudonymizedAuthentified(user) => {
                (AxumStatusCode::OK, Json(user)).into_response()
            }
            UserResponse::Full(user) => (AxumStatusCode::OK, Json(user)).into_response(),
        }
    }
}

fn visible_biography_for_viewer(user: &User, viewer_user_id: Option<Uuid>) -> Option<String> {
    if viewer_user_id == Some(user.id) || user.principal_type != UserPrincipalType::Human {
        user.biography.clone()
    } else {
        None
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
    pub profile_asset_id: Option<Uuid>,
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
    pub home_focus_view: HomeFocusView,
    pub shared_journal_activity_email_mode: EmailNotificationMode,
    pub received_message_email_mode: EmailNotificationMode,
    pub mentor_feedback_email_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles: Option<Vec<UserRole>>,
    pub display_name: String,
}

impl From<&User> for UserPseudonymizedAuthentifiedResponse {
    fn from(user: &User) -> Self {
        Self::from_viewer(user, None)
    }
}

impl UserPseudonymizedAuthentifiedResponse {
    pub(crate) fn from_viewer(user: &User, viewer_user_id: Option<Uuid>) -> Self {
        UserPseudonymizedAuthentifiedResponse {
            id: user.id,
            email: user.email.clone(),
            principal_type: user.principal_type,
            mentor_id: user.mentor_id,
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone(),
            handle: user.handle.clone(),
            created_at: user.created_at,
            updated_at: user.updated_at,
            profile_picture_url: user.profile_picture_url.clone(),
            profile_asset_id: user.profile_asset_id,
            is_platform_user: user.is_platform_user,
            biography: visible_biography_for_viewer(user, viewer_user_id),
            high_level_projects_definition: user.high_level_projects_definition.clone(),
            pseudonymized: user.pseudonymized,
            journal_theme: user.journal_theme,
            current_lens_id: user.current_lens_id,
            week_analysis_weekday: user.week_analysis_weekday,
            timezone: user.timezone.clone(),
            context_anchor_at: user.context_anchor_at,
            welcome_message: user.welcome_message.clone(),
            home_focus_view: user.home_focus_view,
            shared_journal_activity_email_mode: user.shared_journal_activity_email_mode,
            received_message_email_mode: user.received_message_email_mode,
            mentor_feedback_email_enabled: user.mentor_feedback_email_enabled,
            roles: None,
            display_name: user.display_name(),
        }
    }
}

#[derive(Serialize)]
pub struct UserPublicResponse {
    pub id: Uuid,
    pub principal_type: UserPrincipalType,
    pub handle: String,
    pub created_at: NaiveDateTime,
    pub profile_picture_url: Option<String>,
    pub profile_asset_id: Option<Uuid>,
    pub pseudonymized: bool,
    pub welcome_message: Option<String>,
    pub display_name: String,
}

impl From<&User> for UserPublicResponse {
    fn from(user: &User) -> Self {
        UserPublicResponse {
            id: user.id,
            principal_type: user.principal_type,
            handle: user.handle.clone(),
            created_at: user.created_at,
            profile_picture_url: user.profile_picture_url.clone(),
            profile_asset_id: user.profile_asset_id,
            pseudonymized: user.pseudonymized,
            welcome_message: user.welcome_message.clone(),
            display_name: user.display_name(),
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
    pub profile_asset_id: Option<Uuid>,
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
    pub home_focus_view: HomeFocusView,
    pub shared_journal_activity_email_mode: EmailNotificationMode,
    pub received_message_email_mode: EmailNotificationMode,
    pub mentor_feedback_email_enabled: bool,
    pub display_name: String,
}

impl From<&User> for UserPseudonymizedResponse {
    fn from(user: &User) -> Self {
        Self::from_viewer(user, None)
    }
}

impl UserPseudonymizedResponse {
    pub(crate) fn from_viewer(user: &User, viewer_user_id: Option<Uuid>) -> Self {
        UserPseudonymizedResponse {
            id: user.id,
            principal_type: user.principal_type,
            mentor_id: user.mentor_id,
            handle: user.handle.clone(),
            created_at: user.created_at,
            updated_at: user.updated_at,
            profile_picture_url: user.profile_picture_url.clone(),
            profile_asset_id: user.profile_asset_id,
            is_platform_user: user.is_platform_user,
            biography: visible_biography_for_viewer(user, viewer_user_id),
            high_level_projects_definition: user.high_level_projects_definition.clone(),
            pseudonymized: user.pseudonymized,
            journal_theme: user.journal_theme,
            current_lens_id: user.current_lens_id,
            week_analysis_weekday: user.week_analysis_weekday,
            timezone: user.timezone.clone(),
            context_anchor_at: user.context_anchor_at,
            welcome_message: user.welcome_message.clone(),
            home_focus_view: user.home_focus_view,
            shared_journal_activity_email_mode: user.shared_journal_activity_email_mode,
            received_message_email_mode: user.received_message_email_mode,
            mentor_feedback_email_enabled: user.mentor_feedback_email_enabled,
            display_name: user.display_name(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct UserSearchParams {
    pub q: String,
    pub limit: Option<i64>,
}

#[derive(Deserialize, Debug)]
pub struct UserListParams {
    pub principal_type: Option<UserPrincipalType>,
    pub is_platform_user: Option<bool>,
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(QueryableByName)]
pub(crate) struct UserSearchRow {
    #[diesel(sql_type = SqlUuid)]
    pub(crate) id: Uuid,
    #[diesel(sql_type = Text)]
    pub(crate) handle: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) first_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) last_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) profile_picture_url: Option<String>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    pub(crate) profile_asset_id: Option<Uuid>,
    #[diesel(sql_type = Text)]
    pub(crate) principal_type: String,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    pub(crate) pseudonymized: bool,
    #[diesel(sql_type = Text)]
    pub(crate) pseudonym: String,
}

#[derive(Serialize)]
pub struct UserSearchResult {
    pub id: Uuid,
    pub handle: String,
    pub principal_type: UserPrincipalType,
    pub profile_picture_url: Option<String>,
    pub profile_asset_id: Option<Uuid>,
    pub display_name: String,
}

impl From<UserSearchRow> for UserSearchResult {
    fn from(row: UserSearchRow) -> Self {
        let principal_type =
            UserPrincipalType::from_db(&row.principal_type).unwrap_or(UserPrincipalType::Human);
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
            profile_asset_id: row.profile_asset_id,
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
            profile_asset_id: user.profile_asset_id,
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
    pub profile_asset_id: Option<Uuid>,
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
    pub home_focus_view: Option<HomeFocusView>,
    pub shared_journal_activity_email_mode: Option<EmailNotificationMode>,
    pub received_message_email_mode: Option<EmailNotificationMode>,
    pub mentor_feedback_email_enabled: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct NewServiceUserDto {
    pub first_name: String,
    pub last_name: String,
    pub biography: Option<String>,
    pub profile_picture_url: Option<String>,
    pub profile_asset_id: Option<Uuid>,
    pub welcome_message: Option<String>,
}

impl NewServiceUserDto {
    pub(crate) fn to_new_user(self) -> NewUser {
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
            profile_asset_id: self.profile_asset_id,
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
            home_focus_view: None,
            shared_journal_activity_email_mode: None,
            received_message_email_mode: None,
            mentor_feedback_email_enabled: None,
        }
    }

    pub(crate) fn to_service_user_update(self, existing_user: &User) -> NewUser {
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
            profile_asset_id: self.profile_asset_id.or(existing_user.profile_asset_id),
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
            home_focus_view: Some(existing_user.home_focus_view),
            shared_journal_activity_email_mode: Some(
                existing_user.shared_journal_activity_email_mode,
            ),
            received_message_email_mode: Some(existing_user.received_message_email_mode),
            mentor_feedback_email_enabled: Some(existing_user.mentor_feedback_email_enabled),
        }
    }
}

impl NewUser {
    pub fn create(self, pool: &DbPool) -> Result<User, PpdcError> {
        let mut conn = pool.get()?;

        let mut payload = self;
        if payload.principal_type.is_none() {
            payload.principal_type = Some(UserPrincipalType::Human);
        }
        if payload.journal_theme.is_none() {
            payload.journal_theme = Some(JournalTheme::White);
        }
        if payload.home_focus_view.is_none() {
            payload.home_focus_view = Some(HomeFocusView::Follows);
        }
        if payload.shared_journal_activity_email_mode.is_none() {
            payload.shared_journal_activity_email_mode = Some(EmailNotificationMode::Instant);
        }
        if payload.received_message_email_mode.is_none() {
            payload.received_message_email_mode = Some(EmailNotificationMode::Instant);
        }
        if payload.mentor_feedback_email_enabled.is_none() {
            payload.mentor_feedback_email_enabled = Some(true);
        }
        let email = payload.email.clone();

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
        });

        match user {
            Ok(user) => Ok(user),
            Err(error @ DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
                let existing_user = users::table
                    .filter(users::email.eq(&email))
                    .select(User::as_select())
                    .first::<User>(&mut conn)
                    .optional()?;

                if let Some(existing_user) = existing_user {
                    return Err(PpdcError::new(
                        409,
                        ErrorType::ApiError,
                        "User email already exists".to_string(),
                    )
                    .with_details(serde_json::json!({
                        "email": existing_user.email,
                        "created_at": existing_user.created_at.to_string()
                    })));
                }

                Err(error.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn update(self, id: &Uuid, pool: &DbPool) -> Result<User, PpdcError> {
        let mut conn = pool.get()?;

        let result = diesel::update(users::table)
            .filter(users::id.eq(id))
            .set(&self)
            .returning(User::as_returning())
            .get_result(&mut conn)?;
        Ok(result)
    }

    pub fn hash_password(&mut self) -> Result<(), PpdcError> {
        if let Some(password) = &self.password {
            self.password = Some(User::hash_password_value(password)?);
            Ok(())
        } else if self.is_platform_user.is_none() || !self.is_platform_user.unwrap() {
            self.password = Some(String::new());
            Ok(())
        } else {
            Err(PpdcError::new(
                500,
                ErrorType::InternalError,
                "No password to encode".to_string(),
            ))
        }
    }
}

impl User {
    pub fn hash_password_value(password: &str) -> Result<String, PpdcError> {
        let salt: [u8; 32] = rand::thread_rng().gen();
        let config = Config::default();
        argon2::hash_encoded(password.as_bytes(), &salt, &config).map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unable to encode password: {:#?}", err),
            )
        })
    }

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
        let mut conn = pool.get()?;

        let user = users::table
            .filter(users::id.eq(id))
            .select(User::as_select())
            .first(&mut conn)?;
        Ok(user)
    }

    pub fn find_by_username(email: &String, pool: &DbPool) -> Result<User, PpdcError> {
        let mut conn = pool.get()?;
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

        let mut conn = pool.get()?;

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

    pub fn allows_instant_shared_journal_activity_email(&self) -> bool {
        self.shared_journal_activity_email_mode == EmailNotificationMode::Instant
    }

    pub fn allows_instant_received_message_email(&self) -> bool {
        self.received_message_email_mode == EmailNotificationMode::Instant
    }

    pub fn allows_mentor_feedback_email(&self) -> bool {
        self.mentor_feedback_email_enabled
    }

    pub fn list_roles(&self, pool: &DbPool) -> Result<Vec<UserRole>, PpdcError> {
        let mut conn = pool.get()?;

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
        let mut conn = pool.get()?;

        let has_role = diesel::select(diesel::dsl::exists(
            user_roles::table
                .filter(user_roles::user_id.eq(self.id))
                .filter(user_roles::role.eq(role.to_db())),
        ))
        .get_result::<bool>(&mut conn)?;
        Ok(has_role)
    }

    pub fn add_role(&self, role: UserRole, pool: &DbPool) -> Result<(), PpdcError> {
        let mut conn = pool.get()?;

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
    let mut conn = pool.get()?;

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
    let mut conn = pool.get()?;

    let has_autoplay_lens = diesel::select(diesel::dsl::exists(
        lenses::table
            .filter(lenses::user_id.eq(user_id))
            .filter(lenses::autoplay.eq(true)),
    ))
    .get_result::<bool>(&mut conn)?;

    if has_autoplay_lens {
        ensure_user_has_current_lens(user_id, pool)?;
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
    let lens = autoplay_lens.create(pool)?;

    diesel::update(
        users::table
            .filter(users::id.eq(user_id))
            .filter(users::current_lens_id.is_null()),
    )
    .set(users::current_lens_id.eq(Some(lens.id)))
    .execute(&mut conn)?;

    Ok(())
}

fn find_latest_meta_journal_id_for_user(
    user_id: Uuid,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let mut conn = pool.get()?;

    let journal_id = journals::table
        .filter(journals::user_id.eq(user_id))
        .filter(journals::journal_type.eq(JournalType::MetaJournal.to_db()))
        .order(journals::created_at.desc())
        .select(journals::id)
        .first::<Uuid>(&mut conn)
        .optional()?;

    Ok(journal_id)
}

fn find_latest_lens_id_for_user(user_id: Uuid, pool: &DbPool) -> Result<Option<Uuid>, PpdcError> {
    let mut conn = pool.get()?;

    let lens_id = lenses::table
        .filter(lenses::user_id.eq(user_id))
        .order(lenses::created_at.desc())
        .select(lenses::id)
        .first::<Uuid>(&mut conn)
        .optional()?;

    Ok(lens_id)
}

fn ensure_user_has_current_lens(user_id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    let user = User::find(&user_id, pool)?;
    if user.current_lens_id.is_some() {
        return Ok(());
    }

    let Some(latest_lens_id) = find_latest_lens_id_for_user(user_id, pool)? else {
        return Ok(());
    };

    let mut conn = pool.get()?;
    diesel::update(
        users::table
            .filter(users::id.eq(user_id))
            .filter(users::current_lens_id.is_null()),
    )
    .set(users::current_lens_id.eq(Some(latest_lens_id)))
    .execute(&mut conn)?;

    Ok(())
}

pub(crate) fn create_bio_trace_for_user(
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

pub(crate) fn create_high_level_projects_definition_trace_for_user(
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
    let mut conn = pool.get()?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn build_test_user(pseudonymized: bool, principal_type: UserPrincipalType) -> User {
        User {
            id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            principal_type,
            mentor_id: None,
            first_name: "Ada".to_string(),
            last_name: "Lovelace".to_string(),
            handle: "@ada".to_string(),
            password: "hashed-password".to_string(),
            created_at: Utc::now().naive_utc(),
            updated_at: None,
            profile_picture_url: None,
            profile_asset_id: None,
            is_platform_user: false,
            biography: Some("Biography".to_string()),
            pseudonym: "Analytical Poet".to_string(),
            pseudonymized,
            high_level_projects_definition: None,
            journal_theme: JournalTheme::Classic,
            current_lens_id: None,
            week_analysis_weekday: WeekAnalysisWeekday::Monday,
            timezone: "Europe/Monaco".to_string(),
            context_anchor_at: None,
            welcome_message: None,
            home_focus_view: HomeFocusView::Follows,
            shared_journal_activity_email_mode: EmailNotificationMode::Instant,
            received_message_email_mode: EmailNotificationMode::Instant,
            mentor_feedback_email_enabled: true,
        }
    }

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
            profile_asset_id: None,
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
            home_focus_view: None,
            shared_journal_activity_email_mode: None,
            received_message_email_mode: None,
            mentor_feedback_email_enabled: None,
        };
        user.hash_password().unwrap();
        assert_ne!(user.password, Some(String::from("password")));
    }

    #[test]
    fn test_user_public_response_uses_real_name_as_display_name() {
        let user = build_test_user(false, UserPrincipalType::Human);
        let response = UserPublicResponse::from(&user);
        assert_eq!(response.display_name, "Ada Lovelace");
    }

    #[test]
    fn test_user_public_response_uses_pseudonym_as_display_name() {
        let user = build_test_user(true, UserPrincipalType::Human);
        let response = UserPublicResponse::from(&user);
        assert_eq!(response.display_name, "Analytical Poet");
    }

    #[test]
    fn test_user_pseudonymized_authenticated_response_uses_real_name_as_display_name() {
        let user = build_test_user(false, UserPrincipalType::Human);
        let response = UserPseudonymizedAuthentifiedResponse::from(&user);
        assert_eq!(response.display_name, "Ada Lovelace");
    }

    #[test]
    fn test_user_pseudonymized_authenticated_response_uses_pseudonym_as_display_name() {
        let user = build_test_user(true, UserPrincipalType::Human);
        let response = UserPseudonymizedAuthentifiedResponse::from(&user);
        assert_eq!(response.display_name, "Analytical Poet");
    }

    #[test]
    fn test_user_pseudonymized_authenticated_response_shows_human_biography_for_self() {
        let user = build_test_user(false, UserPrincipalType::Human);
        let response = UserPseudonymizedAuthentifiedResponse::from_viewer(&user, Some(user.id));
        assert_eq!(response.biography, Some("Biography".to_string()));
    }

    #[test]
    fn test_user_pseudonymized_response_shows_service_biography() {
        let user = build_test_user(false, UserPrincipalType::Service);
        let response = UserPseudonymizedResponse::from(&user);
        assert_eq!(response.biography, Some("Biography".to_string()));
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
