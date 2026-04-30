use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    session::Session,
};
use crate::pagination::{PaginatedResponse, PaginationParams};
use crate::schema::users;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use chrono::{Duration, NaiveDate, Utc};
use chrono_tz::Tz;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Date, Text, Uuid as SqlUuid};
use serde::Serialize;
use uuid::Uuid;

use super::enums::{UserPrincipalType, UserRole};
use super::model::{
    ensure_user_has_any_lens, ensure_user_has_meta_journal, NewServiceUserDto, User,
};

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
struct PlatformDailyOverviewRow {
    #[diesel(sql_type = Date)]
    day: NaiveDate,
    #[diesel(sql_type = BigInt)]
    written_traces_count: i64,
    #[diesel(sql_type = BigInt)]
    published_posts_count: i64,
    #[diesel(sql_type = BigInt)]
    followed_journal_opened_count: i64,
    #[diesel(sql_type = BigInt)]
    users_count: i64,
    #[diesel(sql_type = BigInt)]
    users_with_written_trace_last_30_days_count: i64,
    #[diesel(sql_type = BigInt)]
    users_with_followed_journal_open_last_30_days_count: i64,
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
    #[diesel(sql_type = BigInt)]
    home_visited_count: i64,
    #[diesel(sql_type = BigInt)]
    history_visited_count: i64,
    #[diesel(sql_type = BigInt)]
    journal_opened_count: i64,
    #[diesel(sql_type = BigInt)]
    followed_journal_opened_count: i64,
    #[diesel(sql_type = BigInt)]
    post_opened_count: i64,
    #[diesel(sql_type = BigInt)]
    feedback_opened_count: i64,
    #[diesel(sql_type = BigInt)]
    summary_opened_count: i64,
    #[diesel(sql_type = BigInt)]
    generated_mentor_feedback_count: i64,
}

#[derive(Serialize)]
pub struct AdminUserDailyActivity {
    pub day: NaiveDate,
    pub written_traces: i64,
    pub created_elements: i64,
    pub created_landmarks: i64,
    pub home_visited_count: i64,
    pub history_visited_count: i64,
    pub journal_opened_count: i64,
    pub followed_journal_opened_count: i64,
    pub post_opened_count: i64,
    pub feedback_opened_count: i64,
    pub summary_opened_count: i64,
    pub generated_mentor_feedback_count: i64,
}

#[derive(Serialize)]
pub struct AdminUserRecentActivity {
    pub id: Uuid,
    pub display_name: String,
    pub has_mentor: bool,
    pub has_current_lens: bool,
    pub draft_posts_count: i64,
    pub hlp_landmarks_count: i64,
    pub failed_lenses_count: i64,
    pub heatmap: Vec<AdminUserDailyActivity>,
}

#[derive(Serialize)]
pub struct AdminPlatformCurrentHealth {
    pub failed_lenses_count: i64,
    pub failed_analyses_count: i64,
    pub onboarded_users_without_mentor_count: i64,
}

#[derive(Serialize)]
pub struct AdminPlatformDailyOverview {
    pub day: NaiveDate,
    pub written_traces_count: i64,
    pub published_posts_count: i64,
    pub followed_journal_opened_count: i64,
    pub users_count: i64,
    pub users_with_written_trace_last_30_days_count: i64,
    pub users_with_followed_journal_open_last_30_days_count: i64,
}

#[derive(Serialize)]
pub struct AdminPlatformOverview {
    pub current_health: AdminPlatformCurrentHealth,
    pub daily_overview: Vec<AdminPlatformDailyOverview>,
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

fn parse_user_timezone_or_utc(user: &User) -> Tz {
    user.timezone.parse::<Tz>().unwrap_or(chrono_tz::UTC)
}

fn postgres_timezone_name(tz: Tz) -> &'static str {
    match tz {
        chrono_tz::Asia::Saigon => "Asia/Ho_Chi_Minh",
        _ => tz.name(),
    }
}

#[debug_handler]
pub async fn get_admin_recent_user_activity_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<AdminUserRecentActivity>>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let pagination = params.validate()?;

    let mut conn = pool.get()?;

    let total = users::table
        .filter(users::principal_type.eq(UserPrincipalType::Human))
        .filter(users::is_platform_user.eq(true))
        .count()
        .get_result::<i64>(&mut conn)?;

    let user_rows = sql_query(
        r#"
        SELECT u.id AS user_id
        FROM users u
        LEFT JOIN usage_events ue ON ue.user_id = u.id
        WHERE u.principal_type = $1
          AND u.is_platform_user = TRUE
        GROUP BY u.id, u.created_at
        ORDER BY GREATEST(COALESCE(MAX(ue.occurred_at), u.created_at), u.created_at) DESC, u.created_at DESC
        OFFSET $2
        LIMIT $3
        "#,
    )
    .bind::<diesel::sql_types::Text, _>(UserPrincipalType::Human.to_db())
    .bind::<BigInt, _>(pagination.offset)
    .bind::<BigInt, _>(pagination.limit)
    .load::<UserIdRow>(&mut conn)?;

    let mut payload = Vec::with_capacity(user_rows.len());

    for row in user_rows {
        let user = User::find(&row.user_id, &pool)?;
        let tz = parse_user_timezone_or_utc(&user);
        let to = Utc::now().with_timezone(&tz).date_naive();
        let from = to - Duration::days(29);
        let timezone = postgres_timezone_name(tz).to_string();
        let draft_posts_count = sql_query(
            r#"
            SELECT COUNT(*)::bigint AS value
            FROM posts
            WHERE user_id = $1
              AND status = 'DRAFT'
              AND source_trace_id IS NOT NULL
            "#,
        )
        .bind::<SqlUuid, _>(row.user_id)
        .get_result::<CountRow>(&mut conn)?
        .value;

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

        let failed_lenses_count = sql_query(
            r#"
            SELECT COUNT(*)::bigint AS value
            FROM lenses
            WHERE user_id = $1
              AND processing_state = 'FAILED'
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
                      AND timezone($4, COALESCE(t.interaction_date, t.created_at) AT TIME ZONE 'UTC')::date = d.day
                ), 0)::bigint AS trace_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM elements e
                    WHERE e.user_id = $1
                      AND timezone($4, e.created_at AT TIME ZONE 'UTC')::date = d.day
                ), 0)::bigint AS element_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM landmarks l
                    WHERE l.user_id = $1
                      AND timezone($4, l.created_at AT TIME ZONE 'UTC')::date = d.day
                ), 0)::bigint AS landmark_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM usage_events ue
                    WHERE ue.user_id = $1
                      AND ue.event_type = 'HOME_VISITED'
                      AND timezone($4, ue.occurred_at AT TIME ZONE 'UTC')::date = d.day
                ), 0)::bigint AS home_visited_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM usage_events ue
                    WHERE ue.user_id = $1
                      AND ue.event_type = 'HISTORY_VISITED'
                      AND timezone($4, ue.occurred_at AT TIME ZONE 'UTC')::date = d.day
                ), 0)::bigint AS history_visited_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM usage_events ue
                    WHERE ue.user_id = $1
                      AND ue.event_type = 'JOURNAL_OPENED'
                      AND timezone($4, ue.occurred_at AT TIME ZONE 'UTC')::date = d.day
                ), 0)::bigint AS journal_opened_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM usage_events ue
                    WHERE ue.user_id = $1
                      AND ue.event_type = 'FOLLOWED_JOURNAL_OPENED'
                      AND timezone($4, ue.occurred_at AT TIME ZONE 'UTC')::date = d.day
                ), 0)::bigint AS followed_journal_opened_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM usage_events ue
                    WHERE ue.user_id = $1
                      AND ue.event_type = 'POST_OPENED'
                      AND timezone($4, ue.occurred_at AT TIME ZONE 'UTC')::date = d.day
                ), 0)::bigint AS post_opened_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM usage_events ue
                    WHERE ue.user_id = $1
                      AND ue.event_type = 'FEEDBACK_OPENED'
                      AND timezone($4, ue.occurred_at AT TIME ZONE 'UTC')::date = d.day
                ), 0)::bigint AS feedback_opened_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM usage_events ue
                    WHERE ue.user_id = $1
                      AND ue.event_type = 'SUMMARY_OPENED'
                      AND timezone($4, ue.occurred_at AT TIME ZONE 'UTC')::date = d.day
                ), 0)::bigint AS summary_opened_count,
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM messages m
                    WHERE m.recipient_user_id = $1
                      AND m.message_type = 'MENTOR_FEEDBACK'
                      AND timezone($4, m.created_at AT TIME ZONE 'UTC')::date = d.day
                ), 0)::bigint AS generated_mentor_feedback_count
            FROM days d
            ORDER BY d.day
            "#,
        )
        .bind::<SqlUuid, _>(row.user_id)
        .bind::<Date, _>(from)
        .bind::<Date, _>(to)
        .bind::<Text, _>(timezone)
        .load::<UserDailyActivityRow>(&mut conn)?;

        let heatmap = heatmap_rows
            .into_iter()
            .map(|r| AdminUserDailyActivity {
                day: r.day,
                written_traces: r.trace_count,
                created_elements: r.element_count,
                created_landmarks: r.landmark_count,
                home_visited_count: r.home_visited_count,
                history_visited_count: r.history_visited_count,
                journal_opened_count: r.journal_opened_count,
                followed_journal_opened_count: r.followed_journal_opened_count,
                post_opened_count: r.post_opened_count,
                feedback_opened_count: r.feedback_opened_count,
                summary_opened_count: r.summary_opened_count,
                generated_mentor_feedback_count: r.generated_mentor_feedback_count,
            })
            .collect::<Vec<_>>();

        payload.push(AdminUserRecentActivity {
            id: row.user_id,
            display_name: user.display_name(),
            has_mentor: user.mentor_id.is_some(),
            has_current_lens: user.current_lens_id.is_some(),
            draft_posts_count,
            hlp_landmarks_count,
            failed_lenses_count,
            heatmap,
        });
    }

    Ok(Json(PaginatedResponse::new(payload, pagination, total)))
}

#[debug_handler]
pub async fn get_admin_platform_overview_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<AdminPlatformOverview>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let mut conn = pool.get()?;

    let failed_lenses_count = sql_query(
        r#"
        SELECT COUNT(*)::bigint AS value
        FROM lenses
        WHERE processing_state = 'FAILED'
        "#,
    )
    .get_result::<CountRow>(&mut conn)?
    .value;

    let failed_analyses_count = sql_query(
        r#"
        SELECT COUNT(*)::bigint AS value
        FROM landscape_analyses
        WHERE processing_state = 'FAILED'
        "#,
    )
    .get_result::<CountRow>(&mut conn)?
    .value;

    let onboarded_users_without_mentor_count = sql_query(
        r#"
        SELECT COUNT(*)::bigint AS value
        FROM users
        WHERE principal_type = 'HUMAN'
          AND is_platform_user = TRUE
          AND onboarding_version = 1
          AND mentor_id IS NULL
        "#,
    )
    .get_result::<CountRow>(&mut conn)?
    .value;

    let to = Utc::now().date_naive();
    let from = to - Duration::days(29);

    let daily_rows = sql_query(
        r#"
        WITH days AS (
            SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
        )
        SELECT
            d.day AS day,
            COALESCE((
                SELECT COUNT(*)::bigint
                FROM traces t
                WHERE t.finalized_at IS NOT NULL
                  AND t.trace_type IN ('USER_TRACE', 'WORKSPACE_TRACE')
                  AND t.finalized_at::date = d.day
            ), 0)::bigint AS written_traces_count,
            COALESCE((
                SELECT COUNT(*)::bigint
                FROM posts p
                WHERE p.status = 'PUBLISHED'
                  AND COALESCE(p.publishing_date, p.created_at)::date = d.day
            ), 0)::bigint AS published_posts_count,
            COALESCE((
                SELECT COUNT(*)::bigint
                FROM usage_events ue
                WHERE ue.event_type = 'FOLLOWED_JOURNAL_OPENED'
                  AND ue.occurred_at::date = d.day
            ), 0)::bigint AS followed_journal_opened_count,
            COALESCE((
                SELECT COUNT(*)::bigint
                FROM users u
                WHERE u.principal_type = 'HUMAN'
                  AND u.is_platform_user = TRUE
                  AND u.created_at < (d.day + interval '1 day')
            ), 0)::bigint AS users_count,
            COALESCE((
                SELECT COUNT(DISTINCT t.user_id)::bigint
                FROM traces t
                INNER JOIN users u ON u.id = t.user_id
                WHERE u.principal_type = 'HUMAN'
                  AND u.is_platform_user = TRUE
                  AND t.finalized_at IS NOT NULL
                  AND t.trace_type IN ('USER_TRACE', 'WORKSPACE_TRACE')
                  AND t.finalized_at >= (d.day::timestamp - interval '29 days')
                  AND t.finalized_at < (d.day::timestamp + interval '1 day')
            ), 0)::bigint AS users_with_written_trace_last_30_days_count,
            COALESCE((
                SELECT COUNT(DISTINCT ue.user_id)::bigint
                FROM usage_events ue
                INNER JOIN users u ON u.id = ue.user_id
                WHERE u.principal_type = 'HUMAN'
                  AND u.is_platform_user = TRUE
                  AND ue.event_type = 'FOLLOWED_JOURNAL_OPENED'
                  AND ue.occurred_at >= (d.day::timestamp - interval '29 days')
                  AND ue.occurred_at < (d.day::timestamp + interval '1 day')
            ), 0)::bigint AS users_with_followed_journal_open_last_30_days_count
        FROM days d
        ORDER BY d.day
        "#,
    )
    .bind::<Date, _>(from)
    .bind::<Date, _>(to)
    .load::<PlatformDailyOverviewRow>(&mut conn)?;

    let daily_overview = daily_rows
        .into_iter()
        .map(|row| AdminPlatformDailyOverview {
            day: row.day,
            written_traces_count: row.written_traces_count,
            published_posts_count: row.published_posts_count,
            followed_journal_opened_count: row.followed_journal_opened_count,
            users_count: row.users_count,
            users_with_written_trace_last_30_days_count: row
                .users_with_written_trace_last_30_days_count,
            users_with_followed_journal_open_last_30_days_count: row
                .users_with_followed_journal_open_last_30_days_count,
        })
        .collect::<Vec<_>>();

    Ok(Json(AdminPlatformOverview {
        current_health: AdminPlatformCurrentHealth {
            failed_lenses_count,
            failed_analyses_count,
            onboarded_users_without_mentor_count,
        },
        daily_overview,
    }))
}

#[debug_handler]
pub async fn get_admin_service_users_route(
    Query(params): Query<PaginationParams>,
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<User>>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let mut conn = pool.get()?;

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
    if let Err(err) = ensure_user_has_any_lens(created_user.id, &pool) {
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
