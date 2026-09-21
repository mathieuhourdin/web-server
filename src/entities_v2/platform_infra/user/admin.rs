use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    session::Session,
};
use crate::pagination::{parse_repeated_query_param, PaginatedResponse, PaginationParams};
use crate::schema::users;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query, RawQuery},
};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::Tz;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Bool, Date, Double, Nullable, Text, Timestamp, Uuid as SqlUuid};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::{UserPrincipalType, UserRole};
use super::model::{
    ensure_user_has_any_lens, ensure_user_has_meta_journal, NewServiceUserDto, User,
};
use super::purge_user;

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
struct AdminUserLlmCostRow {
    #[diesel(sql_type = Text)]
    model: String,
    #[diesel(sql_type = BigInt)]
    call_count: i64,
    #[diesel(sql_type = BigInt)]
    input_tokens_used: i64,
    #[diesel(sql_type = BigInt)]
    cached_input_tokens_used: i64,
    #[diesel(sql_type = BigInt)]
    reasoning_tokens_used: i64,
    #[diesel(sql_type = BigInt)]
    output_tokens_used: i64,
    #[diesel(sql_type = Double)]
    estimated_cost: f64,
}

#[derive(QueryableByName)]
struct AdminUserRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    email: String,
    #[diesel(sql_type = Text)]
    first_name: String,
    #[diesel(sql_type = Text)]
    last_name: String,
    #[diesel(sql_type = Text)]
    handle: String,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Text)]
    principal_type: String,
    #[diesel(sql_type = Bool)]
    is_platform_user: bool,
    #[diesel(sql_type = Bool)]
    ai_features_enabled: bool,
    #[diesel(sql_type = Bool)]
    ai_features_enabled_by_admin: bool,
    #[diesel(sql_type = BigInt)]
    llm_calls_count: i64,
    #[diesel(sql_type = BigInt)]
    ai_requests_last_24h: i64,
    #[diesel(sql_type = BigInt)]
    ai_requests_last_7d: i64,
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
    feed_visited_count: i64,
    #[diesel(sql_type = BigInt)]
    feed_engaged_30s_count: i64,
    #[diesel(sql_type = BigInt)]
    post_opened_count: i64,
    #[diesel(sql_type = BigInt)]
    users_count: i64,
    #[diesel(sql_type = BigInt)]
    users_with_written_trace_last_30_days_count: i64,
    #[diesel(sql_type = BigInt)]
    users_with_written_trace_last_14_days_count: i64,
    #[diesel(sql_type = BigInt)]
    users_with_read_activity_last_30_days_count: i64,
    #[diesel(sql_type = BigInt)]
    users_with_read_activity_last_7_days_count: i64,
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

#[derive(QueryableByName)]
struct AdminAnalyticsSummaryRow {
    #[diesel(sql_type = BigInt)]
    users: i64,
    #[diesel(sql_type = BigInt)]
    active_writers_30d: i64,
    #[diesel(sql_type = BigInt)]
    active_readers_30d: i64,
    #[diesel(sql_type = BigInt)]
    failed_lenses_count: i64,
    #[diesel(sql_type = BigInt)]
    failed_analyses_count: i64,
    #[diesel(sql_type = BigInt)]
    onboarded_users_without_mentor_count: i64,
}

#[derive(QueryableByName)]
struct AdminAnalyticsDayCountRow {
    #[diesel(sql_type = Date)]
    day: NaiveDate,
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct AdminAnalyticsUserSummaryRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    display_name: String,
    #[diesel(sql_type = Bool)]
    has_mentor: bool,
    #[diesel(sql_type = Bool)]
    has_current_lens: bool,
    #[diesel(sql_type = BigInt)]
    draft_posts_count: i64,
    #[diesel(sql_type = BigInt)]
    hlp_landmarks_count: i64,
    #[diesel(sql_type = BigInt)]
    failed_lenses_count: i64,
    #[diesel(sql_type = BigInt)]
    written_traces_count: i64,
    #[diesel(sql_type = Timestamp)]
    last_activity_at: NaiveDateTime,
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
    pub feed_visited_count: i64,
    pub feed_engaged_30s_count: i64,
    pub post_opened_count: i64,
    pub users_count: i64,
    pub users_with_written_trace_last_30_days_count: i64,
    pub users_with_written_trace_last_14_days_count: i64,
    pub users_with_read_activity_last_30_days_count: i64,
    pub users_with_read_activity_last_7_days_count: i64,
}

#[derive(Serialize)]
pub struct AdminPlatformOverview {
    pub current_health: AdminPlatformCurrentHealth,
    pub daily_overview: Vec<AdminPlatformDailyOverview>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdminPlatformAnalyticsMetric {
    Users,
    WrittenTraces,
    PublishedPosts,
    FollowedJournalOpened,
    FeedVisited,
    #[serde(rename = "feed_engaged_30s")]
    FeedEngaged30s,
    PostOpened,
    #[serde(rename = "active_writers_30d")]
    ActiveWriters30d,
    #[serde(rename = "active_writers_14d")]
    ActiveWriters14d,
    #[serde(rename = "active_readers_30d")]
    ActiveReaders30d,
    #[serde(rename = "active_readers_7d")]
    ActiveReaders7d,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdminUserAnalyticsMetric {
    WrittenTraces,
    CreatedElements,
    CreatedLandmarks,
    GeneratedMentorFeedback,
    HomeVisited,
    HistoryVisited,
    JournalOpened,
    FollowedJournalOpened,
    PostOpened,
    FeedbackOpened,
    SummaryOpened,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AdminAnalyticsUserSort {
    #[default]
    LastActivityDesc,
    WrittenTracesDesc,
}

#[derive(Deserialize)]
pub struct AdminAnalyticsDateRangeQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

#[derive(Deserialize)]
pub struct AdminAnalyticsUsersQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    #[serde(default)]
    pub sort: AdminAnalyticsUserSort,
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Serialize)]
pub struct AdminAnalyticsCurrentVolumes {
    pub users: i64,
    pub active_writers_30d: i64,
    pub active_readers_30d: i64,
}

#[derive(Serialize)]
pub struct AdminAnalyticsSummary {
    pub as_of: DateTime<Utc>,
    pub current_volumes: AdminAnalyticsCurrentVolumes,
    pub current_health: AdminPlatformCurrentHealth,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AdminAnalyticsPoint {
    pub date: NaiveDate,
    pub count: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AdminAnalyticsSeries<T> {
    pub metric: T,
    pub points: Vec<AdminAnalyticsPoint>,
}

#[derive(Serialize)]
pub struct AdminPlatformAnalyticsSeriesResponse {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub series: Vec<AdminAnalyticsSeries<AdminPlatformAnalyticsMetric>>,
}

#[derive(Serialize)]
pub struct AdminUserAnalyticsSeriesResponse {
    pub user_id: Uuid,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub series: Vec<AdminAnalyticsSeries<AdminUserAnalyticsMetric>>,
}

#[derive(Serialize)]
pub struct AdminAnalyticsUserSummary {
    pub id: Uuid,
    pub display_name: String,
    pub has_mentor: bool,
    pub has_current_lens: bool,
    pub draft_posts_count: i64,
    pub hlp_landmarks_count: i64,
    pub failed_lenses_count: i64,
    pub written_traces_count: i64,
    pub last_activity_at: NaiveDateTime,
}

#[derive(Deserialize)]
pub struct AdminUsersQuery {
    pub q: Option<String>,
    pub ai_features_enabled_by_admin: Option<bool>,
    pub principal_type: Option<UserPrincipalType>,
    pub is_platform_user: Option<bool>,
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(Serialize)]
pub struct AdminUserListItem {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub handle: String,
    pub display_name: String,
    pub created_at: NaiveDateTime,
    pub principal_type: UserPrincipalType,
    pub is_platform_user: bool,
    pub ai_features_enabled: bool,
    pub ai_features_enabled_by_admin: bool,
    pub allows_ai_features: bool,
    pub llm_calls_count: i64,
    pub ai_requests_last_24h: i64,
    pub ai_requests_last_7d: i64,
}

#[derive(Deserialize)]
pub struct PatchAdminUserAiFeaturesDto {
    pub ai_features_enabled_by_admin: bool,
}

#[derive(Serialize)]
pub struct AdminUserAiFeaturesResponse {
    pub id: Uuid,
    pub ai_features_enabled: bool,
    pub ai_features_enabled_by_admin: bool,
    pub allows_ai_features: bool,
}

#[derive(Deserialize)]
pub struct AdminUserLlmCostsQuery {
    pub created_at_from: Option<NaiveDateTime>,
    pub created_at_to: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize, Default, PartialEq)]
pub struct AdminUserLlmCostAggregate {
    pub call_count: i64,
    pub input_tokens_used: i64,
    pub cached_input_tokens_used: i64,
    pub reasoning_tokens_used: i64,
    pub output_tokens_used: i64,
    pub estimated_cost: f64,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct AdminUserLlmCostByModel {
    pub model: String,
    #[serde(flatten)]
    pub aggregate: AdminUserLlmCostAggregate,
}

#[derive(Serialize)]
pub struct AdminUserLlmCostsResponse {
    pub user_id: Uuid,
    pub created_at_from: Option<NaiveDateTime>,
    pub created_at_to: Option<NaiveDateTime>,
    pub currency: String,
    pub total: AdminUserLlmCostAggregate,
    pub by_model: Vec<AdminUserLlmCostByModel>,
}

fn aggregate_user_llm_cost_rows(
    rows: Vec<AdminUserLlmCostRow>,
) -> (AdminUserLlmCostAggregate, Vec<AdminUserLlmCostByModel>) {
    let mut total = AdminUserLlmCostAggregate::default();
    let by_model = rows
        .into_iter()
        .map(|row| {
            total.call_count += row.call_count;
            total.input_tokens_used += row.input_tokens_used;
            total.cached_input_tokens_used += row.cached_input_tokens_used;
            total.reasoning_tokens_used += row.reasoning_tokens_used;
            total.output_tokens_used += row.output_tokens_used;
            total.estimated_cost += row.estimated_cost;
            AdminUserLlmCostByModel {
                model: row.model,
                aggregate: AdminUserLlmCostAggregate {
                    call_count: row.call_count,
                    input_tokens_used: row.input_tokens_used,
                    cached_input_tokens_used: row.cached_input_tokens_used,
                    reasoning_tokens_used: row.reasoning_tokens_used,
                    output_tokens_used: row.output_tokens_used,
                    estimated_cost: row.estimated_cost,
                },
            }
        })
        .collect();
    (total, by_model)
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

const ADMIN_ANALYTICS_DEFAULT_DAYS: i64 = 30;
const ADMIN_ANALYTICS_MAX_DAYS: i64 = 366;

fn resolve_admin_analytics_date_range(
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    today: NaiveDate,
) -> Result<(NaiveDate, NaiveDate), PpdcError> {
    let to = to.unwrap_or(today);
    let from = from.unwrap_or(to - Duration::days(ADMIN_ANALYTICS_DEFAULT_DAYS - 1));
    if from > to {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "from must be earlier than or equal to to".to_string(),
        ));
    }
    if to.signed_duration_since(from).num_days() + 1 > ADMIN_ANALYTICS_MAX_DAYS {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            format!(
                "analytics date range must not exceed {} days",
                ADMIN_ANALYTICS_MAX_DAYS
            ),
        ));
    }
    Ok((from, to))
}

fn default_platform_metrics() -> Vec<AdminPlatformAnalyticsMetric> {
    vec![
        AdminPlatformAnalyticsMetric::Users,
        AdminPlatformAnalyticsMetric::ActiveWriters30d,
        AdminPlatformAnalyticsMetric::ActiveReaders30d,
    ]
}

fn default_user_metrics() -> Vec<AdminUserAnalyticsMetric> {
    vec![AdminUserAnalyticsMetric::WrittenTraces]
}

fn deduplicate_metrics<T: Copy + PartialEq>(metrics: Vec<T>) -> Vec<T> {
    metrics.into_iter().fold(Vec::new(), |mut unique, metric| {
        if !unique.contains(&metric) {
            unique.push(metric);
        }
        unique
    })
}

fn load_platform_metric_points(
    metric: AdminPlatformAnalyticsMetric,
    from: NaiveDate,
    to: NaiveDate,
    conn: &mut diesel::PgConnection,
) -> Result<Vec<AdminAnalyticsPoint>, PpdcError> {
    let rows = match metric {
        AdminPlatformAnalyticsMetric::FeedVisited
        | AdminPlatformAnalyticsMetric::FeedEngaged30s
        | AdminPlatformAnalyticsMetric::PostOpened => {
            let event_type = match metric {
                AdminPlatformAnalyticsMetric::FeedVisited => "FEED_VISITED",
                AdminPlatformAnalyticsMetric::FeedEngaged30s => "FEED_ENGAGED_30S",
                AdminPlatformAnalyticsMetric::PostOpened => "POST_OPENED",
                _ => unreachable!(),
            };
            sql_query(
                r#"
                WITH days AS (
                    SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
                ),
                activity AS (
                    SELECT
                        timezone(
                            COALESCE(NULLIF(CASE
                                WHEN u.timezone = 'Asia/Saigon' THEN 'Asia/Ho_Chi_Minh'
                                ELSE u.timezone
                            END, ''), 'UTC'),
                            ue.occurred_at AT TIME ZONE 'UTC'
                        )::date AS day,
                        COUNT(*)::bigint AS count
                    FROM usage_events ue
                    INNER JOIN users u ON u.id = ue.user_id
                    WHERE u.principal_type = 'HUMAN'
                      AND u.is_platform_user = TRUE
                      AND ue.event_type = $3
                      AND ue.occurred_at >= ($1::date - interval '1 day')
                      AND ue.occurred_at < ($2::date + interval '2 days')
                    GROUP BY 1
                )
                SELECT d.day, COALESCE(a.count, 0)::bigint AS count
                FROM days d
                LEFT JOIN activity a ON a.day = d.day
                ORDER BY d.day
                "#,
            )
            .bind::<Date, _>(from)
            .bind::<Date, _>(to)
            .bind::<Text, _>(event_type)
            .load::<AdminAnalyticsDayCountRow>(conn)?
        }
        _ => {
            let query = match metric {
                AdminPlatformAnalyticsMetric::Users => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
                    )
                    SELECT d.day, COUNT(u.id)::bigint AS count
                    FROM days d
                    LEFT JOIN users u
                      ON u.principal_type = 'HUMAN'
                     AND u.is_platform_user = TRUE
                     AND u.created_at < (d.day + interval '1 day')
                    GROUP BY d.day
                    ORDER BY d.day
                    "#
                }
                AdminPlatformAnalyticsMetric::WrittenTraces => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
                    )
                    SELECT d.day, COUNT(t.id)::bigint AS count
                    FROM days d
                    LEFT JOIN traces t
                      ON t.finalized_at >= d.day::timestamp
                     AND t.finalized_at < (d.day + interval '1 day')
                     AND t.trace_type IN ('USER_TRACE', 'WORKSPACE_TRACE')
                    GROUP BY d.day
                    ORDER BY d.day
                    "#
                }
                AdminPlatformAnalyticsMetric::PublishedPosts => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
                    )
                    SELECT d.day, COUNT(p.id)::bigint AS count
                    FROM days d
                    LEFT JOIN posts p
                      ON p.status = 'PUBLISHED'
                     AND COALESCE(p.publishing_date, p.created_at) >= d.day::timestamp
                     AND COALESCE(p.publishing_date, p.created_at) < (d.day + interval '1 day')
                    GROUP BY d.day
                    ORDER BY d.day
                    "#
                }
                AdminPlatformAnalyticsMetric::FollowedJournalOpened => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
                    )
                    SELECT d.day, COUNT(ue.id)::bigint AS count
                    FROM days d
                    LEFT JOIN usage_events ue
                      ON ue.event_type = 'FOLLOWED_JOURNAL_OPENED'
                     AND ue.occurred_at >= d.day::timestamp
                     AND ue.occurred_at < (d.day + interval '1 day')
                    GROUP BY d.day
                    ORDER BY d.day
                    "#
                }
                AdminPlatformAnalyticsMetric::ActiveWriters30d => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
                    )
                    SELECT d.day,
                           COUNT(DISTINCT CASE WHEN u.id IS NOT NULL THEN t.user_id END)::bigint AS count
                    FROM days d
                    LEFT JOIN traces t
                      ON t.finalized_at >= (d.day::timestamp - interval '29 days')
                     AND t.finalized_at < (d.day + interval '1 day')
                     AND t.trace_type IN ('USER_TRACE', 'WORKSPACE_TRACE')
                    LEFT JOIN users u
                      ON u.id = t.user_id
                     AND u.principal_type = 'HUMAN'
                     AND u.is_platform_user = TRUE
                    GROUP BY d.day
                    ORDER BY d.day
                    "#
                }
                AdminPlatformAnalyticsMetric::ActiveWriters14d => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
                    )
                    SELECT d.day,
                           COUNT(DISTINCT CASE WHEN u.id IS NOT NULL THEN t.user_id END)::bigint AS count
                    FROM days d
                    LEFT JOIN traces t
                      ON t.finalized_at >= (d.day::timestamp - interval '13 days')
                     AND t.finalized_at < (d.day + interval '1 day')
                     AND t.trace_type IN ('USER_TRACE', 'WORKSPACE_TRACE')
                    LEFT JOIN users u
                      ON u.id = t.user_id
                     AND u.principal_type = 'HUMAN'
                     AND u.is_platform_user = TRUE
                    GROUP BY d.day
                    ORDER BY d.day
                    "#
                }
                AdminPlatformAnalyticsMetric::ActiveReaders30d => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
                    )
                    SELECT d.day,
                           COUNT(DISTINCT CASE WHEN u.id IS NOT NULL THEN ue.user_id END)::bigint AS count
                    FROM days d
                    LEFT JOIN usage_events ue
                      ON ue.event_type IN ('FOLLOWED_JOURNAL_OPENED', 'POST_OPENED')
                     AND ue.occurred_at >= (d.day::timestamp - interval '29 days')
                     AND ue.occurred_at < (d.day + interval '1 day')
                    LEFT JOIN users u
                      ON u.id = ue.user_id
                     AND u.principal_type = 'HUMAN'
                     AND u.is_platform_user = TRUE
                    GROUP BY d.day
                    ORDER BY d.day
                    "#
                }
                AdminPlatformAnalyticsMetric::ActiveReaders7d => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
                    )
                    SELECT d.day,
                           COUNT(DISTINCT CASE WHEN u.id IS NOT NULL THEN ue.user_id END)::bigint AS count
                    FROM days d
                    LEFT JOIN usage_events ue
                      ON ue.event_type IN ('FOLLOWED_JOURNAL_OPENED', 'POST_OPENED')
                     AND ue.occurred_at >= (d.day::timestamp - interval '6 days')
                     AND ue.occurred_at < (d.day + interval '1 day')
                    LEFT JOIN users u
                      ON u.id = ue.user_id
                     AND u.principal_type = 'HUMAN'
                     AND u.is_platform_user = TRUE
                    GROUP BY d.day
                    ORDER BY d.day
                    "#
                }
                AdminPlatformAnalyticsMetric::FeedVisited
                | AdminPlatformAnalyticsMetric::FeedEngaged30s
                | AdminPlatformAnalyticsMetric::PostOpened => unreachable!(),
            };
            sql_query(query)
                .bind::<Date, _>(from)
                .bind::<Date, _>(to)
                .load::<AdminAnalyticsDayCountRow>(conn)?
        }
    };

    Ok(rows
        .into_iter()
        .map(|row| AdminAnalyticsPoint {
            date: row.day,
            count: row.count,
        })
        .collect())
}

fn load_user_metric_points(
    user_id: Uuid,
    timezone: &str,
    metric: AdminUserAnalyticsMetric,
    from: NaiveDate,
    to: NaiveDate,
    conn: &mut diesel::PgConnection,
) -> Result<Vec<AdminAnalyticsPoint>, PpdcError> {
    let rows = match metric {
        AdminUserAnalyticsMetric::HomeVisited
        | AdminUserAnalyticsMetric::HistoryVisited
        | AdminUserAnalyticsMetric::JournalOpened
        | AdminUserAnalyticsMetric::FollowedJournalOpened
        | AdminUserAnalyticsMetric::PostOpened
        | AdminUserAnalyticsMetric::FeedbackOpened
        | AdminUserAnalyticsMetric::SummaryOpened => {
            let event_type = match metric {
                AdminUserAnalyticsMetric::HomeVisited => "HOME_VISITED",
                AdminUserAnalyticsMetric::HistoryVisited => "HISTORY_VISITED",
                AdminUserAnalyticsMetric::JournalOpened => "JOURNAL_OPENED",
                AdminUserAnalyticsMetric::FollowedJournalOpened => "FOLLOWED_JOURNAL_OPENED",
                AdminUserAnalyticsMetric::PostOpened => "POST_OPENED",
                AdminUserAnalyticsMetric::FeedbackOpened => "FEEDBACK_OPENED",
                AdminUserAnalyticsMetric::SummaryOpened => "SUMMARY_OPENED",
                _ => unreachable!(),
            };
            sql_query(
                r#"
                WITH days AS (
                    SELECT generate_series($2::date, $3::date, interval '1 day')::date AS day
                ),
                activity AS (
                    SELECT
                        timezone($4, ue.occurred_at AT TIME ZONE 'UTC')::date AS day,
                        COUNT(*)::bigint AS count
                    FROM usage_events ue
                    WHERE ue.user_id = $1
                      AND ue.event_type = $5
                      AND ue.occurred_at >= ($2::date - interval '1 day')
                      AND ue.occurred_at < ($3::date + interval '2 days')
                    GROUP BY 1
                )
                SELECT d.day, COALESCE(a.count, 0)::bigint AS count
                FROM days d
                LEFT JOIN activity a ON a.day = d.day
                ORDER BY d.day
                "#,
            )
            .bind::<SqlUuid, _>(user_id)
            .bind::<Date, _>(from)
            .bind::<Date, _>(to)
            .bind::<Text, _>(timezone)
            .bind::<Text, _>(event_type)
            .load::<AdminAnalyticsDayCountRow>(conn)?
        }
        _ => {
            let query = match metric {
                AdminUserAnalyticsMetric::WrittenTraces => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($2::date, $3::date, interval '1 day')::date AS day
                    ),
                    activity AS (
                        SELECT
                            timezone($4, COALESCE(t.interaction_date, t.created_at) AT TIME ZONE 'UTC')::date AS day,
                            COUNT(*)::bigint AS count
                        FROM traces t
                        WHERE t.user_id = $1
                          AND t.finalized_at IS NOT NULL
                          AND COALESCE(t.interaction_date, t.created_at) >= ($2::date - interval '1 day')
                          AND COALESCE(t.interaction_date, t.created_at) < ($3::date + interval '2 days')
                        GROUP BY 1
                    )
                    SELECT d.day, COALESCE(a.count, 0)::bigint AS count
                    FROM days d
                    LEFT JOIN activity a ON a.day = d.day
                    ORDER BY d.day
                    "#
                }
                AdminUserAnalyticsMetric::CreatedElements => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($2::date, $3::date, interval '1 day')::date AS day
                    ),
                    activity AS (
                        SELECT timezone($4, e.created_at AT TIME ZONE 'UTC')::date AS day,
                               COUNT(*)::bigint AS count
                        FROM elements e
                        WHERE e.user_id = $1
                          AND e.created_at >= ($2::date - interval '1 day')
                          AND e.created_at < ($3::date + interval '2 days')
                        GROUP BY 1
                    )
                    SELECT d.day, COALESCE(a.count, 0)::bigint AS count
                    FROM days d LEFT JOIN activity a ON a.day = d.day
                    ORDER BY d.day
                    "#
                }
                AdminUserAnalyticsMetric::CreatedLandmarks => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($2::date, $3::date, interval '1 day')::date AS day
                    ),
                    activity AS (
                        SELECT timezone($4, l.created_at AT TIME ZONE 'UTC')::date AS day,
                               COUNT(*)::bigint AS count
                        FROM landmarks l
                        WHERE l.user_id = $1
                          AND l.created_at >= ($2::date - interval '1 day')
                          AND l.created_at < ($3::date + interval '2 days')
                        GROUP BY 1
                    )
                    SELECT d.day, COALESCE(a.count, 0)::bigint AS count
                    FROM days d LEFT JOIN activity a ON a.day = d.day
                    ORDER BY d.day
                    "#
                }
                AdminUserAnalyticsMetric::GeneratedMentorFeedback => {
                    r#"
                    WITH days AS (
                        SELECT generate_series($2::date, $3::date, interval '1 day')::date AS day
                    ),
                    activity AS (
                        SELECT timezone($4, m.created_at AT TIME ZONE 'UTC')::date AS day,
                               COUNT(*)::bigint AS count
                        FROM messages m
                        WHERE m.recipient_user_id = $1
                          AND m.message_type = 'MENTOR_FEEDBACK'
                          AND m.created_at >= ($2::date - interval '1 day')
                          AND m.created_at < ($3::date + interval '2 days')
                        GROUP BY 1
                    )
                    SELECT d.day, COALESCE(a.count, 0)::bigint AS count
                    FROM days d LEFT JOIN activity a ON a.day = d.day
                    ORDER BY d.day
                    "#
                }
                AdminUserAnalyticsMetric::HomeVisited
                | AdminUserAnalyticsMetric::HistoryVisited
                | AdminUserAnalyticsMetric::JournalOpened
                | AdminUserAnalyticsMetric::FollowedJournalOpened
                | AdminUserAnalyticsMetric::PostOpened
                | AdminUserAnalyticsMetric::FeedbackOpened
                | AdminUserAnalyticsMetric::SummaryOpened => unreachable!(),
            };
            sql_query(query)
                .bind::<SqlUuid, _>(user_id)
                .bind::<Date, _>(from)
                .bind::<Date, _>(to)
                .bind::<Text, _>(timezone)
                .load::<AdminAnalyticsDayCountRow>(conn)?
        }
    };

    Ok(rows
        .into_iter()
        .map(|row| AdminAnalyticsPoint {
            date: row.day,
            count: row.count,
        })
        .collect())
}

#[debug_handler]
pub async fn get_admin_users_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<AdminUsersQuery>,
) -> Result<Json<PaginatedResponse<AdminUserListItem>>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let pagination = params.pagination.validate()?;
    let q = params
        .q
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let principal_type = params
        .principal_type
        .map(|principal_type| principal_type.to_db().to_string());

    let mut conn = pool.get()?;

    let filter_sql = r#"
        FROM users u
        WHERE ($1 IS NULL OR (
            LOWER(u.email) LIKE '%' || LOWER($1) || '%'
            OR LOWER(u.first_name) LIKE '%' || LOWER($1) || '%'
            OR LOWER(u.last_name) LIKE '%' || LOWER($1) || '%'
            OR LOWER(u.handle) LIKE '%' || LOWER($1) || '%'
        ))
          AND ($2 IS NULL OR u.ai_features_enabled_by_admin = $2)
          AND ($3 IS NULL OR u.principal_type = $3)
          AND ($4 IS NULL OR u.is_platform_user = $4)
    "#;

    let total = sql_query(format!("SELECT COUNT(*)::bigint AS value {}", filter_sql))
        .bind::<Nullable<Text>, _>(q.clone())
        .bind::<Nullable<Bool>, _>(params.ai_features_enabled_by_admin)
        .bind::<Nullable<Text>, _>(principal_type.clone())
        .bind::<Nullable<Bool>, _>(params.is_platform_user)
        .get_result::<CountRow>(&mut conn)?
        .value;

    let rows = sql_query(format!(
        r#"
        SELECT
            u.id,
            u.email,
            u.first_name,
            u.last_name,
            u.handle,
            u.created_at,
            u.principal_type,
            u.is_platform_user,
            u.ai_features_enabled,
            u.ai_features_enabled_by_admin,
            COALESCE((
                SELECT COUNT(lc.id)::bigint
                FROM llm_calls lc
                LEFT JOIN landscape_analyses la ON la.id = lc.analysis_id
                WHERE la.user_id = u.id OR lc.user_id = u.id
            ), 0)::bigint AS llm_calls_count,
            (
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM messages m
                    WHERE m.sender_user_id = u.id
                      AND m.message_type IN (
                          'QUESTION',
                          'TAROT_READING_REQUEST',
                          'SHARED_TRACE_EXPLANATION_REQUEST',
                          'SHARED_TRACE_TRANSLATION_REQUEST',
                          'JOURNAL_FEEDBACK_REQUEST'
                      )
                      AND m.created_at >= NOW() - INTERVAL '24 hours'
                ), 0)
                + COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM landscape_analyses la
                    WHERE la.user_id = u.id
                      AND la.landscape_analysis_type IN ('TRACE_INCREMENTAL', 'HLP', 'BIO')
                      AND la.created_at >= NOW() - INTERVAL '24 hours'
                ), 0)
                + COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM usage_events ue
                    WHERE ue.user_id = u.id
                      AND ue.event_type = 'AI_TRANSCRIPTION_REQUESTED'
                      AND ue.occurred_at >= NOW() - INTERVAL '24 hours'
                ), 0)
            )::bigint AS ai_requests_last_24h,
            (
                COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM messages m
                    WHERE m.sender_user_id = u.id
                      AND m.message_type IN (
                          'QUESTION',
                          'TAROT_READING_REQUEST',
                          'SHARED_TRACE_EXPLANATION_REQUEST',
                          'SHARED_TRACE_TRANSLATION_REQUEST',
                          'JOURNAL_FEEDBACK_REQUEST'
                      )
                      AND m.created_at >= NOW() - INTERVAL '7 days'
                ), 0)
                + COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM landscape_analyses la
                    WHERE la.user_id = u.id
                      AND la.landscape_analysis_type IN ('TRACE_INCREMENTAL', 'HLP', 'BIO')
                      AND la.created_at >= NOW() - INTERVAL '7 days'
                ), 0)
                + COALESCE((
                    SELECT COUNT(*)::bigint
                    FROM usage_events ue
                    WHERE ue.user_id = u.id
                      AND ue.event_type = 'AI_TRANSCRIPTION_REQUESTED'
                      AND ue.occurred_at >= NOW() - INTERVAL '7 days'
                ), 0)
            )::bigint AS ai_requests_last_7d
        {}
        ORDER BY u.created_at DESC
        OFFSET $5
        LIMIT $6
        "#,
        filter_sql
    ))
    .bind::<Nullable<Text>, _>(q)
    .bind::<Nullable<Bool>, _>(params.ai_features_enabled_by_admin)
    .bind::<Nullable<Text>, _>(principal_type)
    .bind::<Nullable<Bool>, _>(params.is_platform_user)
    .bind::<BigInt, _>(pagination.offset)
    .bind::<BigInt, _>(pagination.limit)
    .load::<AdminUserRow>(&mut conn)?;

    let items = rows
        .into_iter()
        .map(|row| {
            let principal_type = UserPrincipalType::from_db(&row.principal_type)?;
            Ok(AdminUserListItem {
                id: row.id,
                email: row.email,
                display_name: format!("{} {}", row.first_name, row.last_name),
                first_name: row.first_name,
                last_name: row.last_name,
                handle: row.handle,
                created_at: row.created_at,
                principal_type,
                is_platform_user: row.is_platform_user,
                ai_features_enabled: row.ai_features_enabled,
                ai_features_enabled_by_admin: row.ai_features_enabled_by_admin,
                allows_ai_features: row.ai_features_enabled && row.ai_features_enabled_by_admin,
                llm_calls_count: row.llm_calls_count,
                ai_requests_last_24h: row.ai_requests_last_24h,
                ai_requests_last_7d: row.ai_requests_last_7d,
            })
        })
        .collect::<Result<Vec<_>, PpdcError>>()?;

    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn patch_admin_user_ai_features_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<PatchAdminUserAiFeaturesDto>,
) -> Result<Json<AdminUserAiFeaturesResponse>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let mut conn = pool.get()?;

    diesel::update(users::table.filter(users::id.eq(id)))
        .set(users::ai_features_enabled_by_admin.eq(payload.ai_features_enabled_by_admin))
        .execute(&mut conn)?;

    let user = User::find(&id, &pool)?;
    Ok(Json(AdminUserAiFeaturesResponse {
        id: user.id,
        ai_features_enabled: user.ai_features_enabled,
        ai_features_enabled_by_admin: user.ai_features_enabled_by_admin,
        allows_ai_features: user.allows_ai_features(),
    }))
}

#[debug_handler]
pub async fn get_admin_user_llm_costs_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Query(filters): Query<AdminUserLlmCostsQuery>,
) -> Result<Json<AdminUserLlmCostsResponse>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let _target_user = User::find(&id, &pool)?;
    if filters
        .created_at_from
        .is_some_and(|from| filters.created_at_to.is_some_and(|to| from > to))
    {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "created_at_from must be before or equal to created_at_to".to_string(),
        ));
    }

    let mut conn = pool.get()?;
    let rows = sql_query(
        r#"
        SELECT
            lc.model,
            COUNT(*)::bigint AS call_count,
            COALESCE(SUM(lc.input_tokens_used), 0)::bigint AS input_tokens_used,
            COALESCE(SUM(lc.cached_input_tokens_used), 0)::bigint AS cached_input_tokens_used,
            COALESCE(SUM(lc.reasoning_tokens_used), 0)::bigint AS reasoning_tokens_used,
            COALESCE(SUM(lc.output_tokens_used), 0)::bigint AS output_tokens_used,
            COALESCE(SUM(lc.price), 0)::double precision AS estimated_cost
        FROM llm_calls lc
        LEFT JOIN landscape_analyses la ON la.id = lc.analysis_id
        LEFT JOIN messages m ON m.id = lc.message_id
        WHERE lc.user_id = $1
           OR la.user_id = $1
           OR m.recipient_user_id = $1
          AND ($2 IS NULL OR lc.created_at >= $2)
          AND ($3 IS NULL OR lc.created_at <= $3)
        GROUP BY lc.model
        ORDER BY estimated_cost DESC, lc.model ASC
        "#,
    )
    .bind::<SqlUuid, _>(id)
    .bind::<Nullable<Timestamp>, _>(filters.created_at_from)
    .bind::<Nullable<Timestamp>, _>(filters.created_at_to)
    .load::<AdminUserLlmCostRow>(&mut conn)?;

    let (total, by_model) = aggregate_user_llm_cost_rows(rows);

    Ok(Json(AdminUserLlmCostsResponse {
        user_id: id,
        created_at_from: filters.created_at_from,
        created_at_to: filters.created_at_to,
        currency: "USD".to_string(),
        total,
        by_model,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_user_llm_cost_rows, resolve_admin_analytics_date_range,
        AdminPlatformAnalyticsMetric, AdminUserLlmCostRow,
    };
    use chrono::NaiveDate;

    #[test]
    fn aggregates_user_llm_cost_rows_across_models() {
        let (total, by_model) = aggregate_user_llm_cost_rows(vec![
            AdminUserLlmCostRow {
                model: "gpt-5.6-terra".to_string(),
                call_count: 2,
                input_tokens_used: 100,
                cached_input_tokens_used: 20,
                reasoning_tokens_used: 10,
                output_tokens_used: 40,
                estimated_cost: 0.02,
            },
            AdminUserLlmCostRow {
                model: "gpt-4.1-mini".to_string(),
                call_count: 3,
                input_tokens_used: 200,
                cached_input_tokens_used: 50,
                reasoning_tokens_used: 0,
                output_tokens_used: 60,
                estimated_cost: 0.01,
            },
        ]);

        assert_eq!(total.call_count, 5);
        assert_eq!(total.input_tokens_used, 300);
        assert_eq!(total.cached_input_tokens_used, 70);
        assert_eq!(total.reasoning_tokens_used, 10);
        assert_eq!(total.output_tokens_used, 100);
        assert!((total.estimated_cost - 0.03).abs() < 1e-12);
        assert_eq!(by_model.len(), 2);
    }

    #[test]
    fn analytics_date_range_defaults_to_thirty_inclusive_days() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
        let (from, to) = resolve_admin_analytics_date_range(None, None, today).unwrap();

        assert_eq!(from, NaiveDate::from_ymd_opt(2026, 8, 23).unwrap());
        assert_eq!(to, today);
    }

    #[test]
    fn analytics_date_range_rejects_more_than_one_year() {
        let from = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();

        assert!(resolve_admin_analytics_date_range(Some(from), Some(to), to).is_err());
    }

    #[test]
    fn analytics_metrics_use_snake_case_contract_names() {
        let metric: AdminPlatformAnalyticsMetric =
            serde_json::from_str("\"active_writers_30d\"").unwrap();

        assert_eq!(metric, AdminPlatformAnalyticsMetric::ActiveWriters30d);
        assert_eq!(
            serde_json::to_string(&metric).unwrap(),
            "\"active_writers_30d\""
        );
    }
}

#[debug_handler]
pub async fn delete_admin_user_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, PpdcError> {
    let admin_user = ensure_admin_session_user(&session, &pool)?;
    if admin_user.id == id {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "An administrator cannot delete their own account through the admin route".to_string(),
        ));
    }
    purge_user(id, "admin", &pool).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[debug_handler]
pub async fn get_admin_analytics_summary_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<AdminAnalyticsSummary>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let mut conn = pool.get()?;
    let row = sql_query(
        r#"
        SELECT
            (SELECT COUNT(*)::bigint
             FROM users u
             WHERE u.principal_type = 'HUMAN'
               AND u.is_platform_user = TRUE) AS users,
            (SELECT COUNT(DISTINCT t.user_id)::bigint
             FROM traces t
             INNER JOIN users u ON u.id = t.user_id
             WHERE u.principal_type = 'HUMAN'
               AND u.is_platform_user = TRUE
               AND t.finalized_at IS NOT NULL
               AND t.trace_type IN ('USER_TRACE', 'WORKSPACE_TRACE')
               AND t.finalized_at >= (CURRENT_DATE - interval '29 days')
               AND t.finalized_at < (CURRENT_DATE + interval '1 day')) AS active_writers_30d,
            (SELECT COUNT(DISTINCT ue.user_id)::bigint
             FROM usage_events ue
             INNER JOIN users u ON u.id = ue.user_id
             WHERE u.principal_type = 'HUMAN'
               AND u.is_platform_user = TRUE
               AND ue.event_type IN ('FOLLOWED_JOURNAL_OPENED', 'POST_OPENED')
               AND ue.occurred_at >= (CURRENT_DATE - interval '29 days')
               AND ue.occurred_at < (CURRENT_DATE + interval '1 day')) AS active_readers_30d,
            (SELECT COUNT(*)::bigint
             FROM lenses
             WHERE processing_state = 'FAILED') AS failed_lenses_count,
            (SELECT COUNT(*)::bigint
             FROM landscape_analyses
             WHERE processing_state = 'FAILED') AS failed_analyses_count,
            (SELECT COUNT(*)::bigint
             FROM users u
             WHERE u.principal_type = 'HUMAN'
               AND u.is_platform_user = TRUE
               AND u.onboarding_version = 1
               AND u.mentor_id IS NULL) AS onboarded_users_without_mentor_count
        "#,
    )
    .get_result::<AdminAnalyticsSummaryRow>(&mut conn)?;

    Ok(Json(AdminAnalyticsSummary {
        as_of: Utc::now(),
        current_volumes: AdminAnalyticsCurrentVolumes {
            users: row.users,
            active_writers_30d: row.active_writers_30d,
            active_readers_30d: row.active_readers_30d,
        },
        current_health: AdminPlatformCurrentHealth {
            failed_lenses_count: row.failed_lenses_count,
            failed_analyses_count: row.failed_analyses_count,
            onboarded_users_without_mentor_count: row.onboarded_users_without_mentor_count,
        },
    }))
}

#[debug_handler]
pub async fn get_admin_platform_series_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AdminAnalyticsDateRangeQuery>,
) -> Result<Json<AdminPlatformAnalyticsSeriesResponse>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let (from, to) =
        resolve_admin_analytics_date_range(params.from, params.to, Utc::now().date_naive())?;
    let requested_metrics =
        parse_repeated_query_param::<AdminPlatformAnalyticsMetric>(raw_query.as_deref(), "metric")?;
    let metrics = if requested_metrics.is_empty() {
        default_platform_metrics()
    } else {
        deduplicate_metrics(requested_metrics)
    };

    let mut conn = pool.get()?;
    let mut series = Vec::with_capacity(metrics.len());
    for metric in metrics {
        series.push(AdminAnalyticsSeries {
            metric,
            points: load_platform_metric_points(metric, from, to, &mut conn)?,
        });
    }

    Ok(Json(AdminPlatformAnalyticsSeriesResponse {
        from,
        to,
        series,
    }))
}

#[debug_handler]
pub async fn get_admin_analytics_users_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<AdminAnalyticsUsersQuery>,
) -> Result<Json<PaginatedResponse<AdminAnalyticsUserSummary>>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let pagination = params.pagination.validate()?;
    let (from, to) =
        resolve_admin_analytics_date_range(params.from, params.to, Utc::now().date_naive())?;
    let order_by = match params.sort {
        AdminAnalyticsUserSort::LastActivityDesc => {
            "last_activity_at DESC, written_traces_count DESC, u.created_at DESC"
        }
        AdminAnalyticsUserSort::WrittenTracesDesc => {
            "written_traces_count DESC, last_activity_at DESC, u.created_at DESC"
        }
    };

    let mut conn = pool.get()?;
    let total = users::table
        .filter(users::principal_type.eq(UserPrincipalType::Human))
        .filter(users::is_platform_user.eq(true))
        .count()
        .get_result::<i64>(&mut conn)?;

    let rows = sql_query(format!(
        r#"
        WITH last_usage AS (
            SELECT ue.user_id, MAX(ue.occurred_at) AS last_activity_at
            FROM usage_events ue
            GROUP BY ue.user_id
        ),
        draft_posts AS (
            SELECT p.user_id, COUNT(*)::bigint AS count
            FROM posts p
            WHERE p.status = 'DRAFT'
              AND p.source_trace_id IS NOT NULL
            GROUP BY p.user_id
        ),
        hlp_landmarks AS (
            SELECT l.user_id, COUNT(*)::bigint AS count
            FROM landmarks l
            WHERE l.landmark_type = 'HIGH_LEVEL_PROJECT'
            GROUP BY l.user_id
        ),
        failed_lenses AS (
            SELECT l.user_id, COUNT(*)::bigint AS count
            FROM lenses l
            WHERE l.processing_state = 'FAILED'
            GROUP BY l.user_id
        ),
        written_traces AS (
            SELECT t.user_id, COUNT(*)::bigint AS count
            FROM traces t
            INNER JOIN users trace_user ON trace_user.id = t.user_id
            WHERE t.finalized_at IS NOT NULL
              AND t.trace_type IN ('USER_TRACE', 'WORKSPACE_TRACE')
              AND COALESCE(t.interaction_date, t.created_at) >= ($1::date - interval '1 day')
              AND COALESCE(t.interaction_date, t.created_at) < ($2::date + interval '2 days')
              AND timezone(
                    COALESCE(NULLIF(CASE
                        WHEN trace_user.timezone = 'Asia/Saigon' THEN 'Asia/Ho_Chi_Minh'
                        ELSE trace_user.timezone
                    END, ''), 'UTC'),
                    COALESCE(t.interaction_date, t.created_at) AT TIME ZONE 'UTC'
                  )::date BETWEEN $1::date AND $2::date
            GROUP BY t.user_id
        )
        SELECT
            u.id,
            CASE
                WHEN u.pseudonymized THEN u.pseudonym
                ELSE BTRIM(CONCAT(u.first_name, ' ', u.last_name))
            END AS display_name,
            (u.mentor_id IS NOT NULL) AS has_mentor,
            (u.current_lens_id IS NOT NULL) AS has_current_lens,
            COALESCE(dp.count, 0)::bigint AS draft_posts_count,
            COALESCE(hl.count, 0)::bigint AS hlp_landmarks_count,
            COALESCE(fl.count, 0)::bigint AS failed_lenses_count,
            COALESCE(wt.count, 0)::bigint AS written_traces_count,
            GREATEST(COALESCE(lu.last_activity_at, u.created_at), u.created_at) AS last_activity_at
        FROM users u
        LEFT JOIN last_usage lu ON lu.user_id = u.id
        LEFT JOIN draft_posts dp ON dp.user_id = u.id
        LEFT JOIN hlp_landmarks hl ON hl.user_id = u.id
        LEFT JOIN failed_lenses fl ON fl.user_id = u.id
        LEFT JOIN written_traces wt ON wt.user_id = u.id
        WHERE u.principal_type = 'HUMAN'
          AND u.is_platform_user = TRUE
        ORDER BY {}
        OFFSET $3
        LIMIT $4
        "#,
        order_by
    ))
    .bind::<Date, _>(from)
    .bind::<Date, _>(to)
    .bind::<BigInt, _>(pagination.offset)
    .bind::<BigInt, _>(pagination.limit)
    .load::<AdminAnalyticsUserSummaryRow>(&mut conn)?;

    let items = rows
        .into_iter()
        .map(|row| AdminAnalyticsUserSummary {
            id: row.id,
            display_name: row.display_name,
            has_mentor: row.has_mentor,
            has_current_lens: row.has_current_lens,
            draft_posts_count: row.draft_posts_count,
            hlp_landmarks_count: row.hlp_landmarks_count,
            failed_lenses_count: row.failed_lenses_count,
            written_traces_count: row.written_traces_count,
            last_activity_at: row.last_activity_at,
        })
        .collect();

    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn get_admin_user_series_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<AdminAnalyticsDateRangeQuery>,
) -> Result<Json<AdminUserAnalyticsSeriesResponse>, PpdcError> {
    let _admin_user = ensure_admin_session_user(&session, &pool)?;
    let user = User::find(&id, &pool)?;
    if user.principal_type != UserPrincipalType::Human || !user.is_platform_user {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "Analytics user not found".to_string(),
        ));
    }
    let timezone = parse_user_timezone_or_utc(&user);
    let today = Utc::now().with_timezone(&timezone).date_naive();
    let (from, to) = resolve_admin_analytics_date_range(params.from, params.to, today)?;
    let requested_metrics =
        parse_repeated_query_param::<AdminUserAnalyticsMetric>(raw_query.as_deref(), "metric")?;
    let metrics = if requested_metrics.is_empty() {
        default_user_metrics()
    } else {
        deduplicate_metrics(requested_metrics)
    };

    let mut conn = pool.get()?;
    let mut series = Vec::with_capacity(metrics.len());
    for metric in metrics {
        series.push(AdminAnalyticsSeries {
            metric,
            points: load_user_metric_points(
                id,
                postgres_timezone_name(timezone),
                metric,
                from,
                to,
                &mut conn,
            )?,
        });
    }

    Ok(Json(AdminUserAnalyticsSeriesResponse {
        user_id: id,
        from,
        to,
        series,
    }))
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
                      AND t.finalized_at IS NOT NULL
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
        ),
        platform_users AS (
            SELECT
                u.id,
                CASE
                    WHEN u.timezone = 'Asia/Saigon' THEN 'Asia/Ho_Chi_Minh'
                    ELSE u.timezone
                END AS timezone_name
            FROM users u
            WHERE u.principal_type = 'HUMAN'
              AND u.is_platform_user = TRUE
        ),
        usage_events_local_days AS (
            SELECT
                ue.event_type,
                timezone(
                    COALESCE(NULLIF(pu.timezone_name, ''), 'UTC'),
                    ue.occurred_at AT TIME ZONE 'UTC'
                )::date AS local_day
            FROM usage_events ue
            INNER JOIN platform_users pu ON pu.id = ue.user_id
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
                FROM usage_events_local_days ueld
                WHERE ueld.event_type = 'FEED_VISITED'
                  AND ueld.local_day = d.day
            ), 0)::bigint AS feed_visited_count,
            COALESCE((
                SELECT COUNT(*)::bigint
                FROM usage_events_local_days ueld
                WHERE ueld.event_type = 'FEED_ENGAGED_30S'
                  AND ueld.local_day = d.day
            ), 0)::bigint AS feed_engaged_30s_count,
            COALESCE((
                SELECT COUNT(*)::bigint
                FROM usage_events_local_days ueld
                WHERE ueld.event_type = 'POST_OPENED'
                  AND ueld.local_day = d.day
            ), 0)::bigint AS post_opened_count,
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
                SELECT COUNT(DISTINCT t.user_id)::bigint
                FROM traces t
                INNER JOIN users u ON u.id = t.user_id
                WHERE u.principal_type = 'HUMAN'
                  AND u.is_platform_user = TRUE
                  AND t.finalized_at IS NOT NULL
                  AND t.trace_type IN ('USER_TRACE', 'WORKSPACE_TRACE')
                  AND t.finalized_at >= (d.day::timestamp - interval '13 days')
                  AND t.finalized_at < (d.day::timestamp + interval '1 day')
            ), 0)::bigint AS users_with_written_trace_last_14_days_count,
            COALESCE((
                SELECT COUNT(DISTINCT ue.user_id)::bigint
                FROM usage_events ue
                INNER JOIN users u ON u.id = ue.user_id
                WHERE u.principal_type = 'HUMAN'
                  AND u.is_platform_user = TRUE
                  AND ue.event_type IN ('FOLLOWED_JOURNAL_OPENED', 'POST_OPENED')
                  AND ue.occurred_at >= (d.day::timestamp - interval '29 days')
                  AND ue.occurred_at < (d.day::timestamp + interval '1 day')
            ), 0)::bigint AS users_with_read_activity_last_30_days_count,
            COALESCE((
                SELECT COUNT(DISTINCT ue.user_id)::bigint
                FROM usage_events ue
                INNER JOIN users u ON u.id = ue.user_id
                WHERE u.principal_type = 'HUMAN'
                  AND u.is_platform_user = TRUE
                  AND ue.event_type IN ('FOLLOWED_JOURNAL_OPENED', 'POST_OPENED')
                  AND ue.occurred_at >= (d.day::timestamp - interval '6 days')
                  AND ue.occurred_at < (d.day::timestamp + interval '1 day')
            ), 0)::bigint AS users_with_read_activity_last_7_days_count
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
            feed_visited_count: row.feed_visited_count,
            feed_engaged_30s_count: row.feed_engaged_30s_count,
            post_opened_count: row.post_opened_count,
            users_count: row.users_count,
            users_with_written_trace_last_30_days_count: row
                .users_with_written_trace_last_30_days_count,
            users_with_written_trace_last_14_days_count: row
                .users_with_written_trace_last_14_days_count,
            users_with_read_activity_last_30_days_count: row
                .users_with_read_activity_last_30_days_count,
            users_with_read_activity_last_7_days_count: row
                .users_with_read_activity_last_7_days_count,
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
