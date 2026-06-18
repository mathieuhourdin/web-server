use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use chrono::{NaiveDateTime, Utc};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    session::Session,
};
use crate::schema::{devices, sessions};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    Ios,
    Android,
    Web,
    Desktop,
    Unknown,
}

impl DeviceType {
    fn to_db(self) -> &'static str {
        match self {
            DeviceType::Ios => "IOS",
            DeviceType::Android => "ANDROID",
            DeviceType::Web => "WEB",
            DeviceType::Desktop => "DESKTOP",
            DeviceType::Unknown => "UNKNOWN",
        }
    }

    pub fn from_db_for_session_ttl(value: &str) -> DeviceType {
        match value {
            "IOS" => DeviceType::Ios,
            "ANDROID" => DeviceType::Android,
            "WEB" => DeviceType::Web,
            "DESKTOP" => DeviceType::Desktop,
            _ => DeviceType::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PushProvider {
    Apns,
    Fcm,
    WebPush,
    Expo,
}

impl PushProvider {
    fn to_db(self) -> &'static str {
        match self {
            PushProvider::Apns => "APNS",
            PushProvider::Fcm => "FCM",
            PushProvider::WebPush => "WEB_PUSH",
            PushProvider::Expo => "EXPO",
        }
    }
}

fn device_type_api_value(db_value: &str) -> &str {
    match db_value {
        "IOS" => "ios",
        "ANDROID" => "android",
        "WEB" => "web",
        "DESKTOP" => "desktop",
        _ => "unknown",
    }
}

fn push_provider_api_value(db_value: &str) -> &str {
    match db_value {
        "APNS" => "apns",
        "FCM" => "fcm",
        "WEB_PUSH" => "web_push",
        "EXPO" => "expo",
        _ => "unknown",
    }
}

fn serialize_device_type<S>(value: &String, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(device_type_api_value(value))
}

fn serialize_push_provider<S>(value: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serializer.serialize_some(push_provider_api_value(value)),
        None => serializer.serialize_none(),
    }
}

#[derive(Debug, Clone, Queryable, Selectable, Serialize)]
#[diesel(table_name = crate::schema::devices)]
pub struct Device {
    pub id: Uuid,
    pub user_id: Uuid,
    pub identifier: String,
    #[serde(serialize_with = "serialize_device_type")]
    pub device_type: String,
    pub push_token: Option<String>,
    #[serde(serialize_with = "serialize_push_provider")]
    pub push_provider: Option<String>,
    pub name: Option<String>,
    pub app_version: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub browser_name: Option<String>,
    pub browser_version: Option<String>,
    pub last_seen_at: Option<NaiveDateTime>,
    pub push_token_updated_at: Option<NaiveDateTime>,
    pub revoked_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::devices)]
struct NewDevice {
    user_id: Uuid,
    identifier: String,
    device_type: String,
    push_token: Option<String>,
    push_provider: Option<String>,
    name: Option<String>,
    app_version: Option<String>,
    os_name: Option<String>,
    os_version: Option<String>,
    browser_name: Option<String>,
    browser_version: Option<String>,
    last_seen_at: Option<NaiveDateTime>,
    push_token_updated_at: Option<NaiveDateTime>,
    revoked_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceRegistrationDto {
    pub identifier: String,
    pub device_type: DeviceType,
    pub push_token: Option<String>,
    pub push_provider: Option<PushProvider>,
    pub name: Option<String>,
    pub app_version: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub browser_name: Option<String>,
    pub browser_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PatchCurrentDeviceDto {
    pub identifier: Option<String>,
    pub device_type: Option<DeviceType>,
    pub push_token: Option<Option<String>>,
    pub push_provider: Option<Option<PushProvider>>,
    pub name: Option<Option<String>>,
    pub app_version: Option<Option<String>>,
    pub os_name: Option<Option<String>>,
    pub os_version: Option<Option<String>>,
    pub browser_name: Option<Option<String>>,
    pub browser_version: Option<Option<String>>,
}

#[derive(Debug, Serialize)]
pub struct DeviceRevocationResponse {
    pub id: Uuid,
    pub revoked_at: NaiveDateTime,
}

fn normalized_identifier(identifier: &str) -> Result<String, PpdcError> {
    let identifier = identifier.trim().to_string();
    if identifier.is_empty() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "device.identifier is required".to_string(),
        ));
    }
    Ok(identifier)
}

fn validate_push_pair(
    push_token: Option<&str>,
    push_provider: Option<PushProvider>,
) -> Result<Option<String>, PpdcError> {
    match push_token {
        Some(token) if token.trim().is_empty() => Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "push_token cannot be empty".to_string(),
        )),
        Some(_) if push_provider.is_none() => Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "push_provider is required when push_token is provided".to_string(),
        )),
        Some(_) => Ok(push_provider.map(|provider| provider.to_db().to_string())),
        None => Ok(push_provider.map(|provider| provider.to_db().to_string())),
    }
}

fn clear_duplicate_push_token(
    conn: &mut PgConnection,
    user_id: Uuid,
    current_device_id: Uuid,
    push_token: &str,
    push_provider: &str,
) -> Result<(), diesel::result::Error> {
    let now = Utc::now().naive_utc();
    diesel::update(
        devices::table
            .filter(devices::user_id.eq(user_id))
            .filter(devices::id.ne(current_device_id))
            .filter(devices::revoked_at.is_null())
            .filter(devices::push_token.eq(Some(push_token.to_string())))
            .filter(devices::push_provider.eq(Some(push_provider.to_string()))),
    )
    .set((
        devices::push_token.eq::<Option<String>>(None),
        devices::push_provider.eq::<Option<String>>(None),
        devices::push_token_updated_at.eq(Some(now)),
        devices::updated_at.eq(now),
    ))
    .execute(conn)?;
    Ok(())
}

impl Device {
    pub fn find_active_fcm_targets_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Device>, PpdcError> {
        let mut conn = pool.get()?;
        devices::table
            .filter(devices::user_id.eq(user_id))
            .filter(devices::revoked_at.is_null())
            .filter(devices::push_provider.eq(Some("FCM".to_string())))
            .filter(devices::push_token.is_not_null())
            .order(devices::last_seen_at.desc())
            .load(&mut conn)
            .map_err(Into::into)
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<Device, PpdcError> {
        let mut conn = pool.get()?;
        devices::table
            .filter(devices::id.eq(id))
            .first(&mut conn)
            .map_err(Into::into)
    }

    pub fn find_for_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<Device>, PpdcError> {
        let mut conn = pool.get()?;
        devices::table
            .filter(devices::user_id.eq(user_id))
            .order((devices::revoked_at.asc(), devices::last_seen_at.desc()))
            .load(&mut conn)
            .map_err(Into::into)
    }

    pub fn upsert_for_user(
        user_id: Uuid,
        payload: DeviceRegistrationDto,
        pool: &DbPool,
    ) -> Result<Device, PpdcError> {
        let identifier = normalized_identifier(&payload.identifier)?;
        let push_token = payload
            .push_token
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty());
        let push_provider = validate_push_pair(push_token.as_deref(), payload.push_provider)?;
        let now = Utc::now().naive_utc();
        let mut conn = pool.get()?;

        conn.transaction::<Device, diesel::result::Error, _>(|conn| {
            let new_device = NewDevice {
                user_id,
                identifier,
                device_type: payload.device_type.to_db().to_string(),
                push_token: push_token.clone(),
                push_provider: push_provider.clone(),
                name: payload.name,
                app_version: payload.app_version,
                os_name: payload.os_name,
                os_version: payload.os_version,
                browser_name: payload.browser_name,
                browser_version: payload.browser_version,
                last_seen_at: Some(now),
                push_token_updated_at: push_token.as_ref().map(|_| now),
                revoked_at: None,
            };

            let device = diesel::insert_into(devices::table)
                .values(&new_device)
                .on_conflict((devices::user_id, devices::identifier))
                .do_update()
                .set((
                    devices::device_type.eq(&new_device.device_type),
                    devices::push_token.eq(&new_device.push_token),
                    devices::push_provider.eq(&new_device.push_provider),
                    devices::name.eq(&new_device.name),
                    devices::app_version.eq(&new_device.app_version),
                    devices::os_name.eq(&new_device.os_name),
                    devices::os_version.eq(&new_device.os_version),
                    devices::browser_name.eq(&new_device.browser_name),
                    devices::browser_version.eq(&new_device.browser_version),
                    devices::last_seen_at.eq(Some(now)),
                    devices::push_token_updated_at.eq(if new_device.push_token.is_some() {
                        Some(now)
                    } else {
                        None
                    }),
                    devices::revoked_at.eq::<Option<NaiveDateTime>>(None),
                    devices::updated_at.eq(now),
                ))
                .get_result::<Device>(conn)?;

            if let (Some(token), Some(provider)) = (
                device.push_token.as_deref(),
                device.push_provider.as_deref(),
            ) {
                clear_duplicate_push_token(conn, user_id, device.id, token, provider)?;
            }

            Ok(device)
        })
        .map_err(Into::into)
    }

    pub fn touch(device_id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
        let mut conn = pool.get()?;
        let now = Utc::now().naive_utc();
        diesel::update(devices::table.filter(devices::id.eq(device_id)))
            .set((
                devices::last_seen_at.eq(Some(now)),
                devices::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn clear_push_token(device_id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
        let mut conn = pool.get()?;
        let now = Utc::now().naive_utc();
        diesel::update(devices::table.filter(devices::id.eq(device_id)))
            .set((
                devices::push_token.eq::<Option<String>>(None),
                devices::push_provider.eq::<Option<String>>(None),
                devices::push_token_updated_at.eq(Some(now)),
                devices::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_current(
        session: &Session,
        payload: PatchCurrentDeviceDto,
        pool: &DbPool,
    ) -> Result<Device, PpdcError> {
        let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;

        let device = if let Some(device_id) = session.device_id {
            let device = Device::find(device_id, pool)?;
            if device.user_id != user_id || device.revoked_at.is_some() {
                return Err(PpdcError::unauthorized());
            }
            if let Some(identifier) = payload.identifier.as_deref() {
                let identifier = normalized_identifier(identifier)?;
                if identifier != device.identifier {
                    return Err(PpdcError::new(
                        409,
                        ErrorType::ApiError,
                        "DEVICE_SESSION_MISMATCH".to_string(),
                    ));
                }
            }
            device
        } else {
            let identifier = payload.identifier.clone().ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "identifier is required when current session has no device".to_string(),
                )
            })?;
            let device_type = payload.device_type.ok_or_else(|| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    "device_type is required when current session has no device".to_string(),
                )
            })?;
            let registration = DeviceRegistrationDto {
                identifier,
                device_type,
                push_token: payload.push_token.clone().flatten(),
                push_provider: payload.push_provider.flatten(),
                name: payload.name.clone().flatten(),
                app_version: payload.app_version.clone().flatten(),
                os_name: payload.os_name.clone().flatten(),
                os_version: payload.os_version.clone().flatten(),
                browser_name: payload.browser_name.clone().flatten(),
                browser_version: payload.browser_version.clone().flatten(),
            };
            let device = Device::upsert_for_user(user_id, registration, pool)?;
            Session::attach_device(session.id, device.id, pool)?;
            return Ok(device);
        };

        let mut next_push_token = device.push_token.clone();
        let mut next_push_provider = device.push_provider.clone();
        let mut push_token_changed = false;

        if let Some(push_token) = payload.push_token {
            push_token_changed = true;
            match push_token {
                Some(push_token) => {
                    let push_token = push_token.trim().to_string();
                    let push_provider = payload.push_provider.flatten().ok_or_else(|| {
                        PpdcError::new(
                            400,
                            ErrorType::ApiError,
                            "push_provider is required when push_token is provided".to_string(),
                        )
                    })?;
                    next_push_token = Some(push_token);
                    next_push_provider = Some(push_provider.to_db().to_string());
                }
                None => {
                    next_push_token = None;
                    next_push_provider = None;
                }
            }
        } else if let Some(push_provider) = payload.push_provider {
            next_push_provider = push_provider.map(|provider| provider.to_db().to_string());
        }

        if next_push_token.is_some() && next_push_provider.is_none() {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "push_provider is required when push_token is set".to_string(),
            ));
        }

        let now = Utc::now().naive_utc();
        let mut conn = pool.get()?;
        conn.transaction::<Device, diesel::result::Error, _>(|conn| {
            let updated = diesel::update(devices::table.filter(devices::id.eq(device.id)))
                .set((
                    devices::device_type.eq(payload
                        .device_type
                        .map(|value| value.to_db().to_string())
                        .unwrap_or_else(|| device.device_type.clone())),
                    devices::push_token.eq(&next_push_token),
                    devices::push_provider.eq(&next_push_provider),
                    devices::name.eq(payload.name.unwrap_or(device.name.clone())),
                    devices::app_version
                        .eq(payload.app_version.unwrap_or(device.app_version.clone())),
                    devices::os_name.eq(payload.os_name.unwrap_or(device.os_name.clone())),
                    devices::os_version.eq(payload.os_version.unwrap_or(device.os_version.clone())),
                    devices::browser_name
                        .eq(payload.browser_name.unwrap_or(device.browser_name.clone())),
                    devices::browser_version.eq(payload
                        .browser_version
                        .unwrap_or(device.browser_version.clone())),
                    devices::last_seen_at.eq(Some(now)),
                    devices::push_token_updated_at.eq(if push_token_changed {
                        Some(now)
                    } else {
                        device.push_token_updated_at
                    }),
                    devices::updated_at.eq(now),
                ))
                .get_result::<Device>(conn)?;

            if let (Some(token), Some(provider)) = (
                updated.push_token.as_deref(),
                updated.push_provider.as_deref(),
            ) {
                clear_duplicate_push_token(conn, user_id, updated.id, token, provider)?;
            }

            Ok(updated)
        })
        .map_err(Into::into)
    }

    pub fn revoke_for_user(
        device_id: Uuid,
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<DeviceRevocationResponse, PpdcError> {
        let mut conn = pool.get()?;
        let now = Utc::now().naive_utc();
        conn.transaction::<DeviceRevocationResponse, diesel::result::Error, _>(|conn| {
            let device = diesel::update(
                devices::table
                    .filter(devices::id.eq(device_id))
                    .filter(devices::user_id.eq(user_id)),
            )
            .set((
                devices::revoked_at.eq(Some(now)),
                devices::push_token.eq::<Option<String>>(None),
                devices::push_provider.eq::<Option<String>>(None),
                devices::updated_at.eq(now),
            ))
            .get_result::<Device>(conn)?;

            diesel::update(sessions::table.filter(sessions::device_id.eq(Some(device.id))))
                .set((
                    sessions::revoked_at.eq(Some(now)),
                    sessions::authenticated.eq(false),
                    sessions::updated_at.eq(now),
                ))
                .execute(conn)?;

            Ok(DeviceRevocationResponse {
                id: device.id,
                revoked_at: now,
            })
        })
        .map_err(Into::into)
    }
}

#[debug_handler]
pub async fn get_devices_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<Device>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    Ok(Json(Device::find_for_user(user_id, &pool)?))
}

#[debug_handler]
pub async fn patch_current_device_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<PatchCurrentDeviceDto>,
) -> Result<Json<Device>, PpdcError> {
    Ok(Json(Device::update_current(&session, payload, &pool)?))
}

#[debug_handler]
pub async fn delete_device_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<DeviceRevocationResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    Ok(Json(Device::revoke_for_user(id, user_id, &pool)?))
}
