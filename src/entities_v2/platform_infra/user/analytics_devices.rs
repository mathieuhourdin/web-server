//! Read-only device snapshots. Never select token values or installation identifiers.
use super::admin::ensure_admin_session_user;
use crate::{
    db::DbPool,
    entities_v2::{
        device::{device_type_api_value, push_provider_api_value},
        error::{ErrorType, PpdcError},
        session::Session,
    },
    pagination::{PaginatedResponse, PaginationParams, ValidatedPagination},
    schema::{devices, users},
};
use axum::{
    extract::{Extension, Path, Query},
    Json,
};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Default)]
pub struct DeviceAnalyticsQuery {
    active_within_days: Option<i64>,
}

impl DeviceAnalyticsQuery {
    fn days(&self) -> Result<i64, PpdcError> {
        let days = self.active_within_days.unwrap_or(30);
        if !(1..=366).contains(&days) {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "active_within_days must be between 1 and 366".into(),
            ));
        }
        Ok(days)
    }
}

#[derive(Serialize)]
pub struct DeviceCounts {
    users_count: i64,
    devices_count: i64,
    users_percentage: f64,
    devices_percentage: f64,
}

#[derive(Serialize)]
pub struct PlatformBreakdown {
    device_type: String,
    #[serde(flatten)]
    counts: DeviceCounts,
}

#[derive(Serialize)]
pub struct AppVersionBreakdown {
    device_type: String,
    version: Option<String>,
    #[serde(flatten)]
    counts: DeviceCounts,
}

#[derive(Serialize)]
pub struct SoftwareBreakdown {
    device_type: String,
    name: Option<String>,
    version: Option<String>,
    #[serde(flatten)]
    counts: DeviceCounts,
}

#[derive(Serialize)]
pub struct DeviceAnalytics {
    as_of: DateTime<Utc>,
    active_since: DateTime<Utc>,
    active_within_days: i64,
    total_users: i64,
    active_users: i64,
    active_devices: i64,
    users_without_recent_device: i64,
    multi_platform_users: i64,
    platforms: Vec<PlatformBreakdown>,
    app_versions: Vec<AppVersionBreakdown>,
    operating_systems: Vec<SoftwareBreakdown>,
    browsers: Vec<SoftwareBreakdown>,
}

#[derive(QueryableByName)]
struct BreakdownRow {
    #[diesel(sql_type = Text)]
    category: String,
    #[diesel(sql_type = Nullable<Text>)]
    device_type: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version: Option<String>,
    #[diesel(sql_type = BigInt)]
    users_count: i64,
    #[diesel(sql_type = BigInt)]
    devices_count: i64,
}

fn load_analytics(
    days: i64,
    as_of: DateTime<Utc>,
    conn: &mut PgConnection,
) -> Result<DeviceAnalytics, PpdcError> {
    let active_since = as_of - Duration::days(days);
    // One statement gives every breakdown the same snapshot. Active means a recent
    // authenticated request on a registered, non-revoked device, not an app open.
    let rows = diesel::sql_query("WITH eligible_users AS (
        SELECT u.id FROM analytics_users u
        WHERE u.principal_type='HUMAN' AND u.is_platform_user=TRUE
    ), active_devices AS (
        SELECT d.user_id,
            CASE d.device_type WHEN 'IOS' THEN 'ios' WHEN 'ANDROID' THEN 'android'
                WHEN 'WEB' THEN 'web' WHEN 'DESKTOP' THEN 'desktop' ELSE 'unknown' END AS device_type,
            NULLIF(BTRIM(d.app_version),'') AS app_version,
            NULLIF(BTRIM(d.os_name),'') AS os_name, NULLIF(BTRIM(d.os_version),'') AS os_version,
            NULLIF(BTRIM(d.browser_name),'') AS browser_name, NULLIF(BTRIM(d.browser_version),'') AS browser_version
        FROM devices d JOIN eligible_users u ON u.id=d.user_id
        WHERE d.revoked_at IS NULL AND d.last_seen_at >= $1 AND d.last_seen_at <= $2
    )
    SELECT 'accounts'::text AS category,NULL::text AS device_type,NULL::text AS name,NULL::text AS version,
        COUNT(*)::bigint AS users_count,0::bigint AS devices_count FROM eligible_users
    UNION ALL SELECT 'total',NULL,NULL,NULL,COUNT(DISTINCT user_id),COUNT(*) FROM active_devices
    UNION ALL SELECT 'multi_platform',NULL,NULL,NULL,COUNT(*),0 FROM (
        SELECT user_id FROM active_devices GROUP BY user_id HAVING COUNT(DISTINCT device_type)>1
    ) multiple_platforms
    UNION ALL SELECT 'platform',device_type,NULL,NULL,COUNT(DISTINCT user_id),COUNT(*)
        FROM active_devices GROUP BY device_type
    UNION ALL SELECT 'app_version',device_type,NULL,app_version,COUNT(DISTINCT user_id),COUNT(*)
        FROM active_devices GROUP BY device_type,app_version
    UNION ALL SELECT 'os',device_type,os_name,os_version,COUNT(DISTINCT user_id),COUNT(*)
        FROM active_devices GROUP BY device_type,os_name,os_version
    UNION ALL SELECT 'browser',device_type,browser_name,browser_version,COUNT(DISTINCT user_id),COUNT(*)
        FROM active_devices WHERE device_type='web' OR browser_name IS NOT NULL OR browser_version IS NOT NULL
        GROUP BY device_type,browser_name,browser_version
    ORDER BY category,devices_count DESC,device_type,name NULLS LAST,version NULLS LAST")
        .bind::<Timestamp,_>(active_since.naive_utc())
        .bind::<Timestamp,_>(as_of.naive_utc())
        .load::<BreakdownRow>(conn)?;
    let total_users = rows
        .iter()
        .find(|r| r.category == "accounts")
        .map_or(0, |r| r.users_count);
    let (active_users, active_devices) = rows
        .iter()
        .find(|r| r.category == "total")
        .map_or((0, 0), |r| (r.users_count, r.devices_count));
    let mut result = DeviceAnalytics {
        as_of,
        active_since,
        active_within_days: days,
        total_users,
        active_users,
        active_devices,
        users_without_recent_device: total_users - active_users,
        multi_platform_users: 0,
        platforms: vec![],
        app_versions: vec![],
        operating_systems: vec![],
        browsers: vec![],
    };
    for row in rows {
        let percentage = |count: i64, total: i64| {
            if total == 0 {
                0.0
            } else {
                100.0 * count as f64 / total as f64
            }
        };
        let counts = DeviceCounts {
            users_count: row.users_count,
            devices_count: row.devices_count,
            users_percentage: percentage(row.users_count, active_users),
            devices_percentage: percentage(row.devices_count, active_devices),
        };
        let device_type = row.device_type.unwrap_or_else(|| "unknown".into());
        match row.category.as_str() {
            "multi_platform" => result.multi_platform_users = row.users_count,
            "platform" => result.platforms.push(PlatformBreakdown {
                device_type,
                counts,
            }),
            "app_version" => result.app_versions.push(AppVersionBreakdown {
                device_type,
                version: row.version,
                counts,
            }),
            "os" | "browser" => {
                let item = SoftwareBreakdown {
                    device_type,
                    name: row.name,
                    version: row.version,
                    counts,
                };
                if row.category == "os" {
                    result.operating_systems.push(item)
                } else {
                    result.browsers.push(item)
                }
            }
            _ => {}
        }
    }
    Ok(result)
}

#[derive(QueryableByName)]
struct AdminDeviceRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    device_type: String,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    app_version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    os_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    os_version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    browser_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    browser_version: Option<String>,
    #[diesel(sql_type = Nullable<Timestamp>)]
    last_seen_at: Option<NaiveDateTime>,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Nullable<Timestamp>)]
    revoked_at: Option<NaiveDateTime>,
    #[diesel(sql_type = Bool)]
    has_token: bool,
    #[diesel(sql_type = Nullable<Text>)]
    push_provider: Option<String>,
    #[diesel(sql_type = Nullable<Timestamp>)]
    token_updated_at: Option<NaiveDateTime>,
}

#[derive(Serialize)]
pub struct DevicePushStatus {
    has_token: bool,
    provider: Option<String>,
    token_updated_at: Option<NaiveDateTime>,
}

#[derive(Serialize)]
pub struct AdminDeviceView {
    id: Uuid,
    device_type: String,
    name: Option<String>,
    app_version: Option<String>,
    os_name: Option<String>,
    os_version: Option<String>,
    browser_name: Option<String>,
    browser_version: Option<String>,
    last_seen_at: Option<NaiveDateTime>,
    created_at: NaiveDateTime,
    revoked_at: Option<NaiveDateTime>,
    push: DevicePushStatus,
}

impl From<AdminDeviceRow> for AdminDeviceView {
    fn from(row: AdminDeviceRow) -> Self {
        Self {
            id: row.id,
            device_type: device_type_api_value(&row.device_type).into(),
            name: row.name,
            app_version: row.app_version,
            os_name: row.os_name,
            os_version: row.os_version,
            browser_name: row.browser_name,
            browser_version: row.browser_version,
            last_seen_at: row.last_seen_at,
            created_at: row.created_at,
            revoked_at: row.revoked_at,
            push: DevicePushStatus {
                has_token: row.has_token,
                provider: row
                    .push_provider
                    .as_deref()
                    .map(push_provider_api_value)
                    .map(str::to_owned),
                token_updated_at: row.token_updated_at,
            },
        }
    }
}

fn load_user_devices(
    user_id: Uuid,
    page: ValidatedPagination,
    conn: &mut PgConnection,
) -> Result<PaginatedResponse<AdminDeviceView>, PpdcError> {
    // Unlike aggregate analytics, support inspection includes revoked devices and
    // accounts of every kind. A missing account is distinct from an empty device list.
    if !diesel::select(diesel::dsl::exists(
        users::table.filter(users::id.eq(user_id)),
    ))
    .get_result::<bool>(conn)?
    {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "User not found".into(),
        ));
    }
    let total = devices::table
        .filter(devices::user_id.eq(user_id))
        .count()
        .get_result(conn)?;
    let items = diesel::sql_query(
        "SELECT id,device_type,name,app_version,os_name,os_version,
            browser_name,browser_version,last_seen_at,created_at,revoked_at,
            (push_token IS NOT NULL AND BTRIM(push_token)<>'') AS has_token,
            push_provider,push_token_updated_at AS token_updated_at
            FROM devices WHERE user_id=$1
            ORDER BY last_seen_at DESC NULLS LAST,created_at DESC,id DESC LIMIT $2 OFFSET $3",
    )
    .bind::<SqlUuid, _>(user_id)
    .bind::<BigInt, _>(page.limit)
    .bind::<BigInt, _>(page.offset)
    .load::<AdminDeviceRow>(conn)?
    .into_iter()
    .map(Into::into)
    .collect();
    Ok(PaginatedResponse::new(items, page, total))
}

pub async fn get_admin_device_analytics_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<DeviceAnalyticsQuery>,
) -> Result<Json<DeviceAnalytics>, PpdcError> {
    tokio::task::spawn_blocking(move || {
        ensure_admin_session_user(&session, &pool)?;
        let days = params.days()?;
        let mut conn = pool.get()?;
        load_analytics(days, Utc::now(), &mut conn).map(Json)
    })
    .await
    .map_err(|_| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Device analytics worker failed".into(),
        )
    })?
}

pub async fn get_admin_user_devices_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<AdminDeviceView>>, PpdcError> {
    tokio::task::spawn_blocking(move || {
        ensure_admin_session_user(&session, &pool)?;
        let page = params.validate()?;
        let mut conn = pool.get()?;
        conn.build_transaction()
            .read_only()
            .repeatable_read()
            .run(|conn| load_user_devices(user_id, page, conn))
            .map(Json)
    })
    .await
    .map_err(|_| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Device inspection worker failed".into(),
        )
    })?
}

#[cfg(test)]
mod tests;
