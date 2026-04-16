use crate::db::DbPool;
use crate::entities_v2::{
    error::PpdcError,
    platform_infra::mailer::{self, NewOutboundEmail, OutboundEmailProvider},
    session::Session,
};
use crate::environment;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use chrono::Utc;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Text};
use uuid::Uuid;

use super::enums::UserPrincipalType;
use super::model::{
    create_bio_trace_for_user, create_high_level_projects_definition_trace_for_user,
    ensure_user_has_autoplay_lens, ensure_user_has_meta_journal, NewUser, User, UserListParams,
    UserPseudonymizedAuthentifiedResponse, UserPseudonymizedResponse, UserResponse,
    UserSearchParams, UserSearchResult, UserSearchRow,
};

fn enqueue_new_user_signup_notification_email(
    user: &User,
    pool: &DbPool,
) -> Result<Uuid, PpdcError> {
    let users_url = format!(
        "{}/social/users",
        environment::get_app_base_url().trim_end_matches('/')
    );
    let template = mailer::new_user_signup_email(&user.first_name, &user.last_name, &users_url);
    let email = NewOutboundEmail::new(
        None,
        "NEW_USER_SIGNUP".to_string(),
        Some("USER".to_string()),
        Some(user.id),
        "mathieu.hourdin@gmail.com".to_string(),
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
pub async fn get_users(
    Query(params): Query<UserListParams>,
    Extension(pool): Extension<DbPool>,
) -> Result<Json<Vec<UserPseudonymizedResponse>>, PpdcError> {
    let mut conn = pool.get()?;

    let mut query = crate::schema::users::table.into_boxed();
    if let Some(principal_type) = params.principal_type {
        query = query.filter(crate::schema::users::principal_type.eq(principal_type));
    }
    if let Some(is_platform_user) = params.is_platform_user {
        query = query.filter(crate::schema::users::is_platform_user.eq(is_platform_user));
    }

    let results: Vec<UserPseudonymizedResponse> = query
        .offset(params.pagination.offset())
        .limit(params.pagination.limit())
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

    let mut conn = pool.get()?;

    let rows = sql_query(
        r#"
        SELECT
            id,
            handle,
            NULLIF(first_name, '') AS first_name,
            NULLIF(last_name, '') AS last_name,
            profile_picture_url,
            profile_asset_id,
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
    Query(params): Query<crate::pagination::PaginationParams>,
    Extension(pool): Extension<DbPool>,
) -> Result<Json<Vec<UserPseudonymizedResponse>>, PpdcError> {
    let mut conn = pool.get()?;

    let results: Vec<UserPseudonymizedResponse> = crate::schema::users::table
        .filter(crate::schema::users::principal_type.eq(UserPrincipalType::Service))
        .offset(params.offset())
        .limit(params.limit())
        .order(crate::schema::users::created_at.desc())
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
            let _ = diesel::delete(
                crate::schema::users::table.filter(crate::schema::users::id.eq(created_user.id)),
            )
            .execute(&mut conn);
        }
        return Err(err);
    }
    if let Err(err) = ensure_user_has_autoplay_lens(created_user.id, &pool) {
        if let Ok(mut conn) = pool.get() {
            let _ = diesel::delete(
                crate::schema::users::table.filter(crate::schema::users::id.eq(created_user.id)),
            )
            .execute(&mut conn);
        }
        return Err(err);
    }
    if created_user.principal_type == UserPrincipalType::Human {
        match enqueue_new_user_signup_notification_email(&created_user, &pool) {
            Ok(email_id) => {
                let pool_for_task = pool.clone();
                tokio::spawn(async move {
                    let _ = mailer::process_pending_emails(vec![email_id], &pool_for_task).await;
                });
            }
            Err(err) => {
                tracing::error!(
                    target: "mailer",
                    "new_user_signup_notification_failed user_id={} message={}",
                    created_user.id,
                    err.message
                );
            }
        }
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

    if session_user_id != id && existing_user.is_platform_user {
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
) -> Result<impl axum::response::IntoResponse, PpdcError> {
    let user = User::find(&id, &pool)?;
    let viewer_user_id = session.user_id;
    if !user.is_platform_user || session.user_id.unwrap() == id {
        let mut user_response =
            UserPseudonymizedAuthentifiedResponse::from_viewer(&user, viewer_user_id);
        if session.user_id == Some(id) {
            user_response.roles = Some(user.list_roles(&pool)?);
        }
        Ok(UserResponse::PseudonymizedAuthentified(user_response))
    } else {
        let user_response = UserPseudonymizedResponse::from_viewer(&user, viewer_user_id);
        Ok(UserResponse::Pseudonymized(user_response))
    }
}
