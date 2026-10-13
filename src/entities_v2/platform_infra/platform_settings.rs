use axum::{
    debug_handler,
    extract::{Extension, Json, Query},
};
use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{Jsonb, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        session::Session,
        user::{User, UserRole},
    },
};

const MOBILE_POLICY_KEY: &str = "mobile_update_policy";

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MobilePlatform {
    Ios,
    Android,
}

#[derive(Deserialize)]
pub struct MobilePolicyQuery {
    pub platform: MobilePlatform,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MobileBuildVersion {
    pub version: String,
    pub build: i64,
}

impl Default for MobileBuildVersion {
    fn default() -> Self {
        Self {
            version: "0.0.0".to_string(),
            build: 0,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MobileVersionPolicy {
    pub minimum_required: MobileBuildVersion,
    pub recommended: MobileBuildVersion,
    pub store_url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MobileUpdatePolicy {
    pub ios: MobileVersionPolicy,
    pub android: MobileVersionPolicy,
}

#[derive(Serialize)]
pub struct MobileVersionPolicyResponse {
    pub platform: MobilePlatform,
    #[serde(flatten)]
    pub policy: MobileVersionPolicy,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct AdminMobileUpdatePolicyResponse {
    #[serde(flatten)]
    pub policy: MobileUpdatePolicy,
    pub updated_at: Option<DateTime<Utc>>,
    pub updated_by: Option<Uuid>,
}

#[derive(QueryableByName)]
struct SettingRow {
    #[diesel(sql_type = Jsonb)]
    value: serde_json::Value,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    updated_by: Option<Uuid>,
}

fn invalid(message: &str) -> PpdcError {
    PpdcError::new(400, ErrorType::ApiError, message.to_string())
}

impl MobileUpdatePolicy {
    pub fn validate(&mut self) -> Result<(), PpdcError> {
        for (platform, policy) in [
            (MobilePlatform::Ios, &mut self.ios),
            (MobilePlatform::Android, &mut self.android),
        ] {
            for target in [&mut policy.minimum_required, &mut policy.recommended] {
                target.version = target.version.trim().to_string();
                if target.build < 0 || target.version.is_empty() || target.version.len() > 64 {
                    return Err(invalid("Versions require a non-negative build and a nonempty label of at most 64 bytes"));
                }
            }
            if policy.minimum_required.build > policy.recommended.build {
                return Err(invalid(
                    "minimum_required.build must not exceed recommended.build",
                ));
            }
            policy.store_url = policy
                .store_url
                .take()
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty());
            if policy.recommended.build > 0 && policy.store_url.is_none() {
                return Err(invalid(
                    "A store_url is required when an update policy is enabled",
                ));
            }
            if let Some(raw_url) = &policy.store_url {
                let url = url::Url::parse(raw_url).map_err(|_| invalid("Invalid store_url"))?;
                let expected_host = match platform {
                    MobilePlatform::Ios => "apps.apple.com",
                    MobilePlatform::Android => "play.google.com",
                };
                if url.scheme() != "https"
                    || url.host_str() != Some(expected_host)
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || url.port().is_some()
                {
                    return Err(invalid(
                        "store_url must be an HTTPS URL on the platform's official app store",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn from_row(row: SettingRow) -> Result<AdminMobileUpdatePolicyResponse, PpdcError> {
    let mut policy: MobileUpdatePolicy = serde_json::from_value(row.value)?;
    policy.validate().map_err(|_| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            "Stored mobile update policy is invalid".to_string(),
        )
    })?;
    Ok(AdminMobileUpdatePolicyResponse {
        policy,
        updated_at: Some(row.updated_at.and_utc()),
        updated_by: row.updated_by,
    })
}

fn read_policy(pool: &DbPool) -> Result<AdminMobileUpdatePolicyResponse, PpdcError> {
    let row = diesel::sql_query(
        "SELECT value, updated_at, updated_by FROM platform_settings WHERE key=$1",
    )
    .bind::<Text, _>(MOBILE_POLICY_KEY)
    .get_result::<SettingRow>(&mut pool.get()?)
    .optional()?;
    match row {
        Some(row) => from_row(row),
        None => Ok(AdminMobileUpdatePolicyResponse {
            policy: MobileUpdatePolicy::default(),
            updated_at: None,
            updated_by: None,
        }),
    }
}

fn admin_id(session: &Session, pool: &DbPool) -> Result<Uuid, PpdcError> {
    let id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if !User::find(&id, pool)?.has_role(UserRole::Admin, pool)? {
        return Err(PpdcError::new(
            403,
            ErrorType::ApiError,
            "Admin role required".to_string(),
        ));
    }
    Ok(id)
}

#[debug_handler]
pub async fn get_mobile_version_policy_route(
    Extension(pool): Extension<DbPool>,
    Query(query): Query<MobilePolicyQuery>,
) -> Result<Json<MobileVersionPolicyResponse>, PpdcError> {
    let stored = read_policy(&pool)?;
    let policy = match query.platform {
        MobilePlatform::Ios => stored.policy.ios,
        MobilePlatform::Android => stored.policy.android,
    };
    Ok(Json(MobileVersionPolicyResponse {
        platform: query.platform,
        policy,
        updated_at: stored.updated_at,
    }))
}

#[debug_handler]
pub async fn get_admin_mobile_update_policy_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<AdminMobileUpdatePolicyResponse>, PpdcError> {
    admin_id(&session, &pool)?;
    Ok(Json(read_policy(&pool)?))
}

#[debug_handler]
pub async fn put_admin_mobile_update_policy_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(mut policy): Json<MobileUpdatePolicy>,
) -> Result<Json<AdminMobileUpdatePolicyResponse>, PpdcError> {
    let id = admin_id(&session, &pool)?;
    policy.validate()?;
    let value = serde_json::to_value(policy)?;
    let row = diesel::sql_query("INSERT INTO platform_settings(key,value,updated_by) VALUES($1,$2,$3)
        ON CONFLICT(key) DO UPDATE SET value=EXCLUDED.value,updated_by=EXCLUDED.updated_by,updated_at=NOW()
        RETURNING value,updated_at,updated_by")
        .bind::<Text, _>(MOBILE_POLICY_KEY).bind::<Jsonb, _>(value).bind::<SqlUuid, _>(id)
        .get_result::<SettingRow>(&mut pool.get()?)?;
    Ok(Json(from_row(row)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_are_safe_by_default_and_validate_builds_and_store_links() {
        let mut policy = MobileUpdatePolicy::default();
        assert!(policy.validate().is_ok());
        policy.ios.minimum_required.build = 10;
        assert!(policy.validate().is_err());
        policy.ios.recommended.build = 20;
        assert!(policy.validate().is_err());
        policy.ios.store_url = Some("https://apps.apple.com/fr/app/hupo/id123".to_string());
        assert!(policy.validate().is_ok());
        policy.ios.store_url = Some("https://apps.apple.com.evil.example/app".to_string());
        assert!(policy.validate().is_err());
        policy.ios.store_url = Some("http://apps.apple.com/app".to_string());
        assert!(policy.validate().is_err());
        policy.ios.store_url =
            Some("https://play.google.com/store/apps/details?id=app".to_string());
        assert!(policy.validate().is_err());
        policy.ios = MobileVersionPolicy::default();
        policy.android.recommended.build = -1;
        assert!(policy.validate().is_err());
    }
}
