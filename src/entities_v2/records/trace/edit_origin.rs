use axum::{
    extract::{Extension, Path},
    Json,
};
use diesel::prelude::*;
use diesel::sql_types::{Int4, Nullable, Text, Timestamp};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        session::Session,
    },
};

#[derive(Serialize)]
pub struct TraceEditOrigin {
    pub trace_version: i32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub device: Option<TraceEditDevice>,
}

#[derive(Serialize)]
pub struct TraceEditDevice {
    pub platform: String,
    pub name: Option<String>,
}

#[derive(QueryableByName)]
struct EditOriginRow {
    #[diesel(sql_type = Int4)]
    version_integer: i32,
    #[diesel(sql_type = Timestamp)]
    updated_at: chrono::NaiveDateTime,
    #[diesel(sql_type = Nullable<Text>)]
    platform: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
}

pub async fn get_trace_edit_origin_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<TraceEditOrigin>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut conn = pool.get()?;
    // One snapshot for both version and device. Ownership is enforced in SQL;
    // don't load/serialize Device, which contains private identifiers/tokens.
    let row = diesel::sql_query(
        "SELECT t.version_integer, t.updated_at, d.device_type AS platform, d.name
         FROM traces t LEFT JOIN devices d ON d.id = t.last_edit_device_id
             AND d.user_id = t.user_id
         WHERE t.id = $1 AND t.user_id = $2",
    )
    .bind::<diesel::sql_types::Uuid, _>(id)
    .bind::<diesel::sql_types::Uuid, _>(user_id)
    .get_result::<EditOriginRow>(&mut conn)
    .optional()?
    .ok_or_else(|| PpdcError::new(404, ErrorType::ApiError, "Trace not found".to_string()))?;
    Ok(Json(TraceEditOrigin {
        trace_version: row.version_integer,
        updated_at: row.updated_at.and_utc(),
        device: row.platform.map(|platform| TraceEditDevice {
            platform: match platform.as_str() {
                "IOS" => "ios",
                "ANDROID" => "android",
                "WEB" => "web",
                "DESKTOP" => "desktop",
                _ => "unknown",
            }
            .to_string(),
            name: row.name,
        }),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_contains_only_version_and_display_safe_device_information() {
        let updated_at = "2026-10-11T12:30:00Z"
            .parse::<chrono::DateTime<chrono::Utc>>()
            .unwrap();
        let value = serde_json::to_value(TraceEditOrigin {
            trace_version: 42,
            updated_at,
            device: Some(TraceEditDevice {
                platform: "ios".to_string(),
                name: Some("iPhone".to_string()),
            }),
        })
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "trace_version": 42,
                "updated_at": "2026-10-11T12:30:00Z",
                "device": {"platform": "ios", "name": "iPhone"}
            })
        );
        let unknown = serde_json::to_value(TraceEditOrigin {
            trace_version: 43,
            updated_at,
            device: None,
        })
        .unwrap();
        assert_eq!(
            unknown,
            serde_json::json!({"trace_version": 43, "updated_at": "2026-10-11T12:30:00Z", "device": null})
        );
    }
}
