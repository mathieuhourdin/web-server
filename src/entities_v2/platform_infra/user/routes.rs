use crate::db::DbPool;
use crate::entities_v2::{
    asset::{upload_image_asset_for_user_from_multipart, AssetUploadResponse},
    error::{ErrorType, PpdcError},
    feed::hydrate::count_recent_unread_feed_items,
    journal::Journal,
    message::Message,
    platform_infra::mailer::{self, NewOutboundEmail, OutboundEmailProvider},
    platform_infra::usage_event::{UsageEvent, UsageEventType},
    session::Session,
};
use crate::environment;
use crate::pagination::PaginatedResponse;
use axum::{
    debug_handler,
    extract::{Extension, Json, Multipart, Path, Query},
};
use chrono::{DateTime, Duration, Utc};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Bool, Int4, Nullable, SmallInt, Text, Timestamp, Uuid as SqlUuid};
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use super::enums::UserPrincipalType;
use super::enums::{EmailNotificationMode, HomeFocusView, JournalTheme, WeekAnalysisWeekday};
use super::model::{
    create_bio_trace_for_user, create_high_level_projects_definition_trace_for_user,
    ensure_user_has_any_lens, ensure_user_has_default_journals, ensure_user_has_meta_journal,
    NewUser, User, UserListParams, UserPseudonymizedAuthentifiedResponse,
    UserPseudonymizedResponse, UserPublicResponse, UserResponse, UserSearchParams,
    UserSearchResult, UserSearchRow,
};

#[derive(QueryableByName)]
struct SuggestedUserIdRow {
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
}

#[derive(QueryableByName)]
struct RankedFollowerUserIdRow {
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    total: i64,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct PatchUserDto {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub handle: Option<String>,
    pub profile_picture_url: Option<Option<String>>,
    #[serde(alias = "profile_asset_id")]
    pub profile_picture_asset_id: Option<Option<Uuid>>,
    pub biography: Option<Option<String>>,
    pub pseudonym: Option<String>,
    pub pseudonymized: Option<bool>,
    pub high_level_projects_definition: Option<Option<String>>,
    pub journal_theme: Option<JournalTheme>,
    pub current_lens_id: Option<Option<Uuid>>,
    pub week_analysis_weekday: Option<WeekAnalysisWeekday>,
    pub timezone: Option<String>,
    pub context_anchor_at: Option<Option<chrono::NaiveDateTime>>,
    pub welcome_message: Option<Option<String>>,
    pub home_focus_view: Option<HomeFocusView>,
    pub shared_journal_activity_email_mode: Option<EmailNotificationMode>,
    pub received_message_email_mode: Option<EmailNotificationMode>,
    pub mentor_feedback_email_enabled: Option<bool>,
    pub ai_features_enabled: Option<bool>,
    pub onboarding_version: Option<i32>,
    pub external_captures_default_journal_id: Option<Option<Uuid>>,
}

#[derive(Serialize)]
pub struct MeUnreadCountsResponse {
    pub unread_posts_count: i64,
    pub unread_messages_count: i64,
    pub posts_window_days: i64,
    pub messages_window_days: i64,
    pub as_of: DateTime<Utc>,
}

fn hydrate_user_search_results(
    ids: Vec<Uuid>,
    pool: &DbPool,
) -> Result<Vec<UserSearchResult>, PpdcError> {
    let users = User::find_many(&ids, pool)?;
    let users_by_id = users
        .into_iter()
        .map(|user| (user.id, user))
        .collect::<std::collections::HashMap<Uuid, User>>();

    Ok(ids
        .into_iter()
        .filter_map(|id| users_by_id.get(&id).map(UserSearchResult::from))
        .collect())
}

#[derive(Deserialize, Debug)]
pub struct SuggestedUsersParams {
    pub limit: Option<i64>,
}

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
        "hupo <noreply@ppdcoeur.fr>".to_string(),
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
) -> Result<Json<PaginatedResponse<UserPublicResponse>>, PpdcError> {
    let mut conn = pool.get()?;
    let pagination = params.pagination.validate()?;

    let mut count_query = crate::schema::users::table.into_boxed();
    if let Some(principal_type) = params.principal_type {
        count_query = count_query.filter(crate::schema::users::principal_type.eq(principal_type));
    }
    if let Some(is_platform_user) = params.is_platform_user {
        count_query =
            count_query.filter(crate::schema::users::is_platform_user.eq(is_platform_user));
    }
    let total = count_query.count().get_result::<i64>(&mut conn)?;

    let mut query = crate::schema::users::table.into_boxed();
    if let Some(principal_type) = params.principal_type {
        query = query.filter(crate::schema::users::principal_type.eq(principal_type));
    }
    if let Some(is_platform_user) = params.is_platform_user {
        query = query.filter(crate::schema::users::is_platform_user.eq(is_platform_user));
    }

    let results: Vec<UserPublicResponse> = query
        .offset(pagination.offset)
        .limit(pagination.limit)
        .select(User::as_select())
        .load::<User>(&mut conn)?
        .iter()
        .map(UserPublicResponse::from)
        .collect();
    Ok(Json(PaginatedResponse::new(results, pagination, total)))
}

#[debug_handler]
pub async fn get_user_search_route(
    Query(params): Query<UserSearchParams>,
    Extension(pool): Extension<DbPool>,
) -> Result<Json<PaginatedResponse<UserSearchResult>>, PpdcError> {
    let pagination = params.pagination.validate()?;
    let query = params.q.trim();
    if query.is_empty() {
        return Ok(Json(PaginatedResponse::new(vec![], pagination, 0)));
    }

    let contains_query = format!("%{}%", query);
    let prefix_query = format!("{}%", query);

    let mut conn = pool.get()?;

    let total = sql_query(
        r#"
        SELECT COUNT(*)::bigint AS total
        FROM users
        WHERE principal_type = 'HUMAN'
          AND (
            handle ILIKE $1
            OR first_name ILIKE $1
            OR last_name ILIKE $1
            OR CONCAT_WS(' ', first_name, last_name) ILIKE $1
          )
        "#,
    )
    .bind::<Text, _>(contains_query.clone())
    .get_result::<CountRow>(&mut conn)?
    .total;

    let rows = sql_query(
        r#"
        SELECT
            id,
            handle,
            NULLIF(first_name, '') AS first_name,
            NULLIF(last_name, '') AS last_name,
            profile_picture_url,
            profile_picture_asset_id,
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
        OFFSET $4
        LIMIT $5
        "#,
    )
    .bind::<Text, _>(contains_query)
    .bind::<Text, _>(query.to_string())
    .bind::<Text, _>(prefix_query)
    .bind::<BigInt, _>(pagination.offset)
    .bind::<BigInt, _>(pagination.limit)
    .load::<UserSearchRow>(&mut conn)?;

    Ok(Json(PaginatedResponse::new(
        rows.into_iter().map(UserSearchResult::from).collect(),
        pagination,
        total,
    )))
}

#[debug_handler]
pub async fn get_suggested_users_route(
    Query(params): Query<SuggestedUsersParams>,
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<UserPublicResponse>>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let limit = params.limit.unwrap_or(10).clamp(1, 20);
    let since = Utc::now().naive_utc() - chrono::Duration::days(30);

    let mut conn = pool.get()?;

    let ranked_user_ids = sql_query(
        r#"
        SELECT u.id AS user_id
        FROM users u
        LEFT JOIN posts p
          ON p.user_id = u.id
          AND p.status = 'PUBLISHED'
          AND COALESCE(p.publishing_date, p.created_at) >= $2
        WHERE u.principal_type = 'HUMAN'
          AND u.is_platform_user = TRUE
          AND u.id <> $1
          AND NOT EXISTS (
            SELECT 1
            FROM relationships r
            WHERE r.relationship_type = 'FOLLOW'
              AND r.status IN ('PENDING', 'ACCEPTED')
              AND (
                (r.requester_user_id = $1 AND r.target_user_id = u.id)
                OR
                (r.requester_user_id = u.id AND r.target_user_id = $1)
              )
          )
        GROUP BY u.id, u.created_at
        ORDER BY COUNT(p.id) DESC, COALESCE(MAX(COALESCE(p.publishing_date, p.created_at)), u.created_at) DESC, u.created_at DESC
        LIMIT $3
        "#,
    )
    .bind::<SqlUuid, _>(session_user_id)
    .bind::<Timestamp, _>(since)
    .bind::<BigInt, _>(limit)
    .load::<SuggestedUserIdRow>(&mut conn)?;

    let results = ranked_user_ids
        .into_iter()
        .map(|row| User::find(&row.user_id, &pool).map(|user| UserPublicResponse::from(&user)))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(results))
}

#[debug_handler]
pub async fn get_me_unread_counts_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<MeUnreadCountsResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let as_of = Utc::now();
    let posts_window_days = 7;
    let messages_window_days = 14;
    let posts_window_floor = (as_of - Duration::days(posts_window_days)).naive_utc();
    let posts_cutoff = UsageEvent::find_latest_occurred_at_for_user_and_type(
        user_id,
        UsageEventType::FeedEngaged30s,
        &pool,
    )?
    .map(|occurred_at| occurred_at.max(posts_window_floor))
    .unwrap_or(posts_window_floor);

    let unread_posts_count = count_recent_unread_feed_items(user_id, posts_cutoff, &pool)?;
    let unread_messages_count = Message::count_recent_unread_received_messages(
        user_id,
        (as_of - Duration::days(messages_window_days)).naive_utc(),
        &pool,
    )?;

    Ok(Json(MeUnreadCountsResponse {
        unread_posts_count,
        unread_messages_count,
        posts_window_days,
        messages_window_days,
        as_of,
    }))
}

#[debug_handler]
pub async fn get_closest_followers_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<crate::pagination::PaginationParams>,
) -> Result<Json<PaginatedResponse<UserSearchResult>>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if session_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let pagination = params.validate()?;
    let mut conn = pool.get()?;

    let total = sql_query(
        r#"
        SELECT COUNT(*)::bigint AS total
        FROM relationships r
        WHERE r.target_user_id = $1
          AND r.relationship_type = 'FOLLOW'
          AND r.status = 'ACCEPTED'
        "#,
    )
    .bind::<SqlUuid, _>(user_id)
    .get_result::<CountRow>(&mut conn)?
    .total;

    let ranked_user_ids = sql_query(
        r#"
        WITH follower_candidates AS (
            SELECT
                r.requester_user_id AS follower_user_id,
                r.accepted_at
            FROM relationships r
            WHERE r.target_user_id = $1
              AND r.relationship_type = 'FOLLOW'
              AND r.status = 'ACCEPTED'
        ),
        message_stats AS (
            SELECT
                fc.follower_user_id,
                COUNT(*) FILTER (
                    WHERE m.created_at >= NOW() - INTERVAL '30 days'
                ) AS recent_messages_count
            FROM follower_candidates fc
            LEFT JOIN messages m
                ON (
                    (m.sender_user_id = $1 AND m.recipient_user_id = fc.follower_user_id)
                    OR
                    (m.sender_user_id = fc.follower_user_id AND m.recipient_user_id = $1)
                )
            GROUP BY fc.follower_user_id
        ),
        post_open_stats AS (
            SELECT
                fc.follower_user_id,
                COUNT(*) FILTER (
                    WHERE ue.occurred_at >= NOW() - INTERVAL '30 days'
                ) AS recent_post_open_count
            FROM follower_candidates fc
            LEFT JOIN posts p
                ON p.user_id = fc.follower_user_id
               AND p.status = 'PUBLISHED'
            LEFT JOIN usage_events ue
                ON ue.resource_id = p.id
               AND ue.user_id = $1
               AND ue.event_type = 'POST_OPENED'
            GROUP BY fc.follower_user_id
        )
        SELECT fc.follower_user_id AS user_id
        FROM follower_candidates fc
        LEFT JOIN message_stats ms
            ON ms.follower_user_id = fc.follower_user_id
        LEFT JOIN post_open_stats pos
            ON pos.follower_user_id = fc.follower_user_id
        ORDER BY
            (
                5 * COALESCE(ms.recent_messages_count, 0)
                + 4 * COALESCE(pos.recent_post_open_count, 0)
            ) DESC,
            fc.accepted_at ASC NULLS LAST,
            fc.follower_user_id ASC
        OFFSET $2
        LIMIT $3
        "#,
    )
    .bind::<SqlUuid, _>(user_id)
    .bind::<BigInt, _>(pagination.offset)
    .bind::<BigInt, _>(pagination.limit)
    .load::<RankedFollowerUserIdRow>(&mut conn)?;

    let ids = ranked_user_ids.into_iter().map(|row| row.user_id).collect();
    let results = hydrate_user_search_results(ids, &pool)?;

    Ok(Json(PaginatedResponse::new(results, pagination, total)))
}

#[debug_handler]
pub async fn get_mentors_route(
    Query(params): Query<crate::pagination::PaginationParams>,
    Extension(pool): Extension<DbPool>,
) -> Result<Json<PaginatedResponse<UserPseudonymizedResponse>>, PpdcError> {
    let mut conn = pool.get()?;
    let pagination = params.validate()?;

    let total = crate::schema::users::table
        .filter(crate::schema::users::principal_type.eq(UserPrincipalType::Service))
        .count()
        .get_result::<i64>(&mut conn)?;

    let results: Vec<UserPseudonymizedResponse> = crate::schema::users::table
        .filter(crate::schema::users::principal_type.eq(UserPrincipalType::Service))
        .offset(pagination.offset)
        .limit(pagination.limit)
        .order(crate::schema::users::created_at.desc())
        .select(User::as_select())
        .load::<User>(&mut conn)?
        .iter()
        .map(UserPseudonymizedResponse::from)
        .collect();
    Ok(Json(PaginatedResponse::new(results, pagination, total)))
}

#[debug_handler]
pub async fn post_user(
    Extension(pool): Extension<DbPool>,
    Json(mut payload): Json<NewUser>,
) -> Result<Json<User>, PpdcError> {
    payload.hash_password().unwrap();
    let created_user = payload.create(&pool)?;
    if let Err(err) = ensure_user_has_default_journals(created_user.id, &pool) {
        if let Ok(mut conn) = pool.get() {
            let _ = diesel::delete(
                crate::schema::users::table.filter(crate::schema::users::id.eq(created_user.id)),
            )
            .execute(&mut conn);
        }
        return Err(err);
    }
    if let Err(err) = ensure_user_has_meta_journal(created_user.id, &pool) {
        if let Ok(mut conn) = pool.get() {
            let _ = diesel::delete(
                crate::schema::users::table.filter(crate::schema::users::id.eq(created_user.id)),
            )
            .execute(&mut conn);
        }
        return Err(err);
    }
    if let Err(err) = ensure_user_has_any_lens(created_user.id, &pool) {
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
    if session_user_id != id {
        return Err(PpdcError::unauthorized());
    }

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

    if let Some(requested_onboarding_version) = payload.onboarding_version {
        User::validate_onboarding_version_transition(
            existing_user.onboarding_version,
            requested_onboarding_version,
        )?;
    }
    if let Some(journal_id) = payload.external_captures_default_journal_id {
        let journal = Journal::find_full(journal_id, &pool)?;
        if journal.user_id != id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "external_captures_default_journal_id must reference one of your journals"
                    .to_string(),
            ));
        }
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
pub async fn patch_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<PatchUserDto>,
) -> Result<Json<User>, PpdcError> {
    let session_user_id = session.user_id.unwrap();
    if session_user_id != id {
        return Err(PpdcError::unauthorized());
    }

    let existing_user = User::find(&id, &pool)?;
    if let Some(requested_onboarding_version) = payload.onboarding_version {
        User::validate_onboarding_version_transition(
            existing_user.onboarding_version,
            requested_onboarding_version,
        )?;
    }
    if let Some(Some(journal_id)) = payload.external_captures_default_journal_id {
        let journal = Journal::find_full(journal_id, &pool)?;
        if journal.user_id != id {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "external_captures_default_journal_id must reference one of your journals"
                    .to_string(),
            ));
        }
    }

    let patched_biography = payload
        .biography
        .clone()
        .unwrap_or_else(|| existing_user.biography.clone());
    let biography_changed =
        payload.biography.is_some() && patched_biography != existing_user.biography;
    let patched_hlp_definition = payload
        .high_level_projects_definition
        .clone()
        .unwrap_or_else(|| existing_user.high_level_projects_definition.clone());
    let hlp_definition_changed = payload.high_level_projects_definition.is_some()
        && patched_hlp_definition != existing_user.high_level_projects_definition;

    let mut conn = pool.get()?;
    diesel::sql_query(
        "UPDATE users
         SET first_name = COALESCE($2, first_name),
             last_name = COALESCE($3, last_name),
             handle = COALESCE($4, handle),
             profile_picture_url = CASE WHEN $5 THEN $6 ELSE profile_picture_url END,
             profile_picture_asset_id = CASE WHEN $7 THEN $8 ELSE profile_picture_asset_id END,
             biography = CASE WHEN $9 THEN $10 ELSE biography END,
             pseudonym = COALESCE($11, pseudonym),
             pseudonymized = COALESCE($12, pseudonymized),
             high_level_projects_definition = CASE WHEN $13 THEN $14 ELSE high_level_projects_definition END,
             journal_theme = COALESCE($15, journal_theme),
             current_lens_id = CASE WHEN $16 THEN $17 ELSE current_lens_id END,
             week_analysis_weekday = COALESCE($18, week_analysis_weekday),
             timezone = COALESCE($19, timezone),
             context_anchor_at = CASE WHEN $20 THEN $21 ELSE context_anchor_at END,
             welcome_message = CASE WHEN $22 THEN $23 ELSE welcome_message END,
             home_focus_view = COALESCE($24, home_focus_view),
             shared_journal_activity_email_mode = COALESCE($25, shared_journal_activity_email_mode),
             received_message_email_mode = COALESCE($26, received_message_email_mode),
             mentor_feedback_email_enabled = COALESCE($27, mentor_feedback_email_enabled),
             ai_features_enabled = COALESCE($28, ai_features_enabled),
             onboarding_version = COALESCE($29, onboarding_version),
             external_captures_default_journal_id = CASE WHEN $30 THEN $31 ELSE external_captures_default_journal_id END,
             updated_at = NOW()
         WHERE id = $1
         ",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<Nullable<Text>, _>(payload.first_name)
    .bind::<Nullable<Text>, _>(payload.last_name)
    .bind::<Nullable<Text>, _>(payload.handle)
    .bind::<Bool, _>(payload.profile_picture_url.is_some())
    .bind::<Nullable<Text>, _>(payload.profile_picture_url.flatten())
    .bind::<Bool, _>(payload.profile_picture_asset_id.is_some())
    .bind::<Nullable<SqlUuid>, _>(payload.profile_picture_asset_id.flatten())
    .bind::<Bool, _>(payload.biography.is_some())
    .bind::<Nullable<Text>, _>(patched_biography.clone())
    .bind::<Nullable<Text>, _>(payload.pseudonym)
    .bind::<Nullable<Bool>, _>(payload.pseudonymized)
    .bind::<Bool, _>(payload.high_level_projects_definition.is_some())
    .bind::<Nullable<Text>, _>(patched_hlp_definition.clone())
    .bind::<Nullable<Text>, _>(payload.journal_theme.map(|value| value.to_code().to_string()))
    .bind::<Bool, _>(payload.current_lens_id.is_some())
    .bind::<Nullable<SqlUuid>, _>(payload.current_lens_id.flatten())
    .bind::<Nullable<SmallInt>, _>(
        payload
            .week_analysis_weekday
            .map(|value| value.to_db_value()),
    )
    .bind::<Nullable<Text>, _>(payload.timezone)
    .bind::<Bool, _>(payload.context_anchor_at.is_some())
    .bind::<Nullable<Timestamp>, _>(payload.context_anchor_at.flatten())
    .bind::<Bool, _>(payload.welcome_message.is_some())
    .bind::<Nullable<Text>, _>(payload.welcome_message.flatten())
    .bind::<Nullable<Text>, _>(payload.home_focus_view.map(|value| value.to_code().to_string()))
    .bind::<Nullable<Text>, _>(
        payload
            .shared_journal_activity_email_mode
            .map(|value| value.to_code().to_string()),
    )
    .bind::<Nullable<Text>, _>(
        payload
            .received_message_email_mode
            .map(|value| value.to_code().to_string()),
    )
    .bind::<Nullable<Bool>, _>(payload.mentor_feedback_email_enabled)
    .bind::<Nullable<Bool>, _>(payload.ai_features_enabled)
    .bind::<Nullable<Int4>, _>(payload.onboarding_version)
    .bind::<Bool, _>(payload.external_captures_default_journal_id.is_some())
    .bind::<Nullable<SqlUuid>, _>(payload.external_captures_default_journal_id.flatten())
    .execute(&mut conn)?;
    let updated_user = User::find(&id, &pool)?;

    if biography_changed {
        if let Some(biography) = patched_biography {
            create_bio_trace_for_user(id, biography, &pool)?;
        }
    }
    if hlp_definition_changed {
        if let Some(high_level_projects_definition) = patched_hlp_definition {
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
pub async fn post_user_profile_picture_asset_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<AssetUploadResponse>, PpdcError> {
    let session_user_id = session.user_id.unwrap();
    if session_user_id != id {
        return Err(PpdcError::unauthorized());
    }

    let response = upload_image_asset_for_user_from_multipart(id, &pool, multipart).await?;
    let mut conn = pool.get()?;
    diesel::sql_query(
        "UPDATE users
         SET profile_picture_asset_id = $2,
             profile_picture_url = NULL,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<SqlUuid, _>(response.asset.id)
    .execute(&mut conn)?;

    Ok(Json(response))
}

#[debug_handler]
pub async fn get_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, PpdcError> {
    let user = User::find(&id, &pool)?;
    let viewer_user_id = session.user_id;
    if session.user_id == Some(id) {
        let mut user_response =
            UserPseudonymizedAuthentifiedResponse::from_viewer(&user, viewer_user_id);
        if session.user_id == Some(id) {
            user_response.roles = Some(user.list_roles(&pool)?);
        }
        Ok(UserResponse::PseudonymizedAuthentified(user_response))
    } else {
        let user_response = UserPublicResponse::from(&user);
        Ok(UserResponse::Public(user_response))
    }
}
