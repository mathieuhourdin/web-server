use axum::{
    debug_handler,
    extract::{Extension, Json, Multipart, Path},
};
use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Nullable, Text, Timestamp, Uuid as SqlUuid};
use serde::de::Deserializer;
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    platform_infra::asset::{upload_image_asset_for_user_from_multipart, AssetUploadResponse},
    session::Session,
    trace::{Trace, TraceSharingSensitivity, TraceStatus},
};
use crate::schema::trace_versions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceVersionStatus {
    Draft,
    Finalized,
    Archived,
}

impl TraceVersionStatus {
    pub fn to_code(self) -> &'static str {
        match self {
            TraceVersionStatus::Draft => "draft",
            TraceVersionStatus::Finalized => "finalized",
            TraceVersionStatus::Archived => "archived",
        }
    }

    pub fn from_code(value: &str) -> Self {
        match value {
            "finalized" | "FINALIZED" | "Finalized" => TraceVersionStatus::Finalized,
            "archived" | "ARCHIVED" | "Archived" => TraceVersionStatus::Archived,
            _ => TraceVersionStatus::Draft,
        }
    }

    pub fn to_db(self) -> &'static str {
        match self {
            TraceVersionStatus::Draft => "DRAFT",
            TraceVersionStatus::Finalized => "FINALIZED",
            TraceVersionStatus::Archived => "ARCHIVED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        Self::from_code(value)
    }
}

impl Serialize for TraceVersionStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_code())
    }
}

impl<'de> Deserialize<'de> for TraceVersionStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from_code(&value))
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct TraceVersion {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub version_number: Option<i32>,
    pub status: TraceVersionStatus,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub content_image_asset_id: Option<Uuid>,
    pub sharing_sensitivity: TraceSharingSensitivity,
    pub interaction_date: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub finalized_at: Option<NaiveDateTime>,
}

#[derive(Deserialize)]
pub struct UpdateTraceVersionDto {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<String>,
    #[serde(alias = "image_asset_id")]
    pub content_image_asset_id: Option<Option<Uuid>>,
    pub sharing_sensitivity: Option<TraceSharingSensitivity>,
    pub interaction_date: Option<NaiveDateTime>,
    pub status: Option<TraceVersionStatus>,
}

#[derive(Serialize)]
pub struct TraceVersionAssetUploadResponse {
    pub trace_version: TraceVersion,
    pub asset: crate::entities_v2::asset::Asset,
    pub signed_url: String,
    pub expires_at: NaiveDateTime,
}

type TraceVersionTuple = (
    Uuid,
    Uuid,
    Option<i32>,
    String,
    String,
    String,
    String,
    Option<Uuid>,
    String,
    NaiveDateTime,
    NaiveDateTime,
    NaiveDateTime,
    Option<NaiveDateTime>,
);

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

fn tuple_to_trace_version(row: TraceVersionTuple) -> TraceVersion {
    TraceVersion {
        id: row.0,
        trace_id: row.1,
        version_number: row.2,
        status: TraceVersionStatus::from_db(&row.3),
        title: row.4,
        subtitle: row.5,
        content: row.6,
        content_image_asset_id: row.7,
        sharing_sensitivity: TraceSharingSensitivity::from_db(&row.8),
        interaction_date: row.9,
        created_at: row.10,
        updated_at: row.11,
        finalized_at: row.12,
    }
}

fn select_trace_version_columns() -> (
    trace_versions::id,
    trace_versions::trace_id,
    trace_versions::version_number,
    trace_versions::status,
    trace_versions::title,
    trace_versions::subtitle,
    trace_versions::content,
    trace_versions::content_image_asset_id,
    trace_versions::sharing_sensitivity,
    trace_versions::interaction_date,
    trace_versions::created_at,
    trace_versions::updated_at,
    trace_versions::finalized_at,
) {
    (
        trace_versions::id,
        trace_versions::trace_id,
        trace_versions::version_number,
        trace_versions::status,
        trace_versions::title,
        trace_versions::subtitle,
        trace_versions::content,
        trace_versions::content_image_asset_id,
        trace_versions::sharing_sensitivity,
        trace_versions::interaction_date,
        trace_versions::created_at,
        trace_versions::updated_at,
        trace_versions::finalized_at,
    )
}

impl TraceVersion {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = pool.get()?;
        let row = trace_versions::table
            .filter(trace_versions::id.eq(id))
            .select(select_trace_version_columns())
            .first::<TraceVersionTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_trace_version).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Trace version not found".to_string(),
            )
        })
    }

    pub fn find_draft_for_trace(trace_id: Uuid, pool: &DbPool) -> Result<Option<Self>, PpdcError> {
        let mut conn = pool.get()?;
        let row = trace_versions::table
            .filter(trace_versions::trace_id.eq(trace_id))
            .filter(trace_versions::status.eq(TraceVersionStatus::Draft.to_db()))
            .select(select_trace_version_columns())
            .first::<TraceVersionTuple>(&mut conn)
            .optional()?;
        Ok(row.map(tuple_to_trace_version))
    }

    pub fn get_or_create_draft_for_trace(
        trace_id: Uuid,
        owner_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Self, PpdcError> {
        let trace = Trace::find_full_trace(trace_id, pool)?;
        if trace.user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }
        if trace.status == TraceStatus::Archived {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot create a draft version for an archived trace".to_string(),
            ));
        }
        if let Some(draft) = Self::find_draft_for_trace(trace_id, pool)? {
            return Ok(draft);
        }

        let mut conn = pool.get()?;
        let row = diesel::sql_query(
            "INSERT INTO trace_versions (
                id,
                trace_id,
                version_number,
                status,
                title,
                subtitle,
                content,
                content_image_asset_id,
                sharing_sensitivity,
                interaction_date,
                created_at,
                updated_at,
                finalized_at
             ) VALUES (
                uuid_generate_v4(),
                $1,
                NULL,
                'DRAFT',
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                NOW(),
                NOW(),
                NULL
             )
             RETURNING id",
        )
        .bind::<SqlUuid, _>(trace.id)
        .bind::<Text, _>(&trace.title)
        .bind::<Text, _>(&trace.subtitle)
        .bind::<Text, _>(&trace.content)
        .bind::<Nullable<SqlUuid>, _>(trace.content_image_asset_id)
        .bind::<Text, _>(trace.sharing_sensitivity.to_db())
        .bind::<Timestamp, _>(trace.interaction_date)
        .get_result::<IdRow>(&mut conn)?;

        Self::find(row.id, pool)
    }

    fn save_draft(self, pool: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = pool.get()?;
        conn.transaction::<(), diesel::result::Error, _>(|conn| {
            diesel::update(trace_versions::table.filter(trace_versions::id.eq(self.id)))
                .set((
                    trace_versions::title.eq(&self.title),
                    trace_versions::subtitle.eq(&self.subtitle),
                    trace_versions::content.eq(&self.content),
                    trace_versions::content_image_asset_id.eq(self.content_image_asset_id),
                    trace_versions::sharing_sensitivity.eq(self.sharing_sensitivity.to_db()),
                    trace_versions::interaction_date.eq(self.interaction_date),
                    trace_versions::updated_at.eq(Utc::now().naive_utc()),
                ))
                .execute(conn)?;

            diesel::sql_query(
                "UPDATE traces
                 SET title = $2,
                     subtitle = $3,
                     content = $4,
                     content_image_asset_id = $5,
                     sharing_sensitivity = $6,
                     interaction_date = $7,
                     updated_at = NOW()
                 WHERE id = $1
                   AND status = 'DRAFT'",
            )
            .bind::<SqlUuid, _>(self.trace_id)
            .bind::<Text, _>(&self.title)
            .bind::<Text, _>(&self.subtitle)
            .bind::<Text, _>(&self.content)
            .bind::<Nullable<SqlUuid>, _>(self.content_image_asset_id)
            .bind::<Text, _>(self.sharing_sensitivity.to_db())
            .bind::<Timestamp, _>(self.interaction_date)
            .execute(conn)?;

            diesel::sql_query(
                "UPDATE journals
                 SET last_trace_at = (
                    SELECT MAX(interaction_date)
                    FROM traces
                    WHERE journal_id = (
                        SELECT journal_id FROM traces WHERE id = $1
                    )
                 ),
                 updated_at = NOW()
                 WHERE id = (
                    SELECT journal_id FROM traces WHERE id = $1
                 )
                   AND EXISTS (
                    SELECT 1 FROM traces WHERE id = $1 AND status = 'DRAFT'
                 )",
            )
            .bind::<SqlUuid, _>(self.trace_id)
            .execute(conn)?;

            Ok(())
        })?;
        Self::find(self.id, pool)
    }

    fn finalize(self, pool: &DbPool) -> Result<Self, PpdcError> {
        if self.content.trim().is_empty() {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot finalize an empty trace version".to_string(),
            ));
        }

        let mut conn = pool.get()?;
        let finalized_at = Utc::now().naive_utc();
        let version_id = self.id;
        let trace_id = self.trace_id;

        conn.transaction::<(), diesel::result::Error, _>(|conn| {
            let next_version_number = trace_versions::table
                .filter(trace_versions::trace_id.eq(trace_id))
                .filter(trace_versions::status.eq(TraceVersionStatus::Finalized.to_db()))
                .select(diesel::dsl::max(trace_versions::version_number))
                .first::<Option<i32>>(conn)?
                .unwrap_or(0)
                + 1;

            diesel::update(trace_versions::table.filter(trace_versions::id.eq(version_id)))
                .set((
                    trace_versions::status.eq(TraceVersionStatus::Finalized.to_db()),
                    trace_versions::version_number.eq(Some(next_version_number)),
                    trace_versions::title.eq(&self.title),
                    trace_versions::subtitle.eq(&self.subtitle),
                    trace_versions::content.eq(&self.content),
                    trace_versions::content_image_asset_id.eq(self.content_image_asset_id),
                    trace_versions::sharing_sensitivity.eq(self.sharing_sensitivity.to_db()),
                    trace_versions::interaction_date.eq(self.interaction_date),
                    trace_versions::updated_at.eq(finalized_at),
                    trace_versions::finalized_at.eq(Some(finalized_at)),
                ))
                .execute(conn)?;

            diesel::sql_query(
                "UPDATE traces
                 SET title = $2,
                     subtitle = $3,
                     content = $4,
                     content_image_asset_id = $5,
                     sharing_sensitivity = $6,
                     interaction_date = $7,
                     status = 'FINALIZED',
                     finalized_at = $8,
                     timeout_at = NULL,
                     current_version_id = $9,
                     updated_at = NOW()
                 WHERE id = $1",
            )
            .bind::<SqlUuid, _>(trace_id)
            .bind::<Text, _>(&self.title)
            .bind::<Text, _>(&self.subtitle)
            .bind::<Text, _>(&self.content)
            .bind::<Nullable<SqlUuid>, _>(self.content_image_asset_id)
            .bind::<Text, _>(self.sharing_sensitivity.to_db())
            .bind::<Timestamp, _>(self.interaction_date)
            .bind::<Timestamp, _>(finalized_at)
            .bind::<SqlUuid, _>(version_id)
            .execute(conn)?;

            diesel::sql_query(
                "UPDATE journals
                 SET last_trace_at = (
                    SELECT MAX(interaction_date)
                    FROM traces
                    WHERE journal_id = (
                        SELECT journal_id FROM traces WHERE id = $1
                    )
                 ),
                 updated_at = NOW()
                 WHERE id = (
                    SELECT journal_id FROM traces WHERE id = $1
                 )",
            )
            .bind::<SqlUuid, _>(trace_id)
            .execute(conn)?;

            Ok(())
        })?;

        Self::find(version_id, pool)
    }

    pub fn update_from_payload(
        mut self,
        payload: UpdateTraceVersionDto,
        pool: &DbPool,
    ) -> Result<Self, PpdcError> {
        if self.status != TraceVersionStatus::Draft {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Only draft trace versions can be updated".to_string(),
            ));
        }

        if let Some(title) = payload.title {
            self.title = title;
        }
        if let Some(subtitle) = payload.subtitle {
            self.subtitle = subtitle;
        }
        if let Some(content) = payload.content {
            self.content = content;
        }
        if let Some(content_image_asset_id) = payload.content_image_asset_id {
            self.content_image_asset_id = content_image_asset_id;
        }
        if let Some(sharing_sensitivity) = payload.sharing_sensitivity {
            self.sharing_sensitivity = sharing_sensitivity;
        }
        if let Some(interaction_date) = payload.interaction_date {
            self.interaction_date = interaction_date;
        }

        match payload.status.unwrap_or(TraceVersionStatus::Draft) {
            TraceVersionStatus::Draft => self.save_draft(pool),
            TraceVersionStatus::Finalized => self.finalize(pool),
            TraceVersionStatus::Archived => Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Use DELETE to discard a draft trace version".to_string(),
            )),
        }
    }

    pub fn delete_draft(self, pool: &DbPool) -> Result<Self, PpdcError> {
        if self.status != TraceVersionStatus::Draft {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Only draft trace versions can be deleted".to_string(),
            ));
        }
        let mut conn = pool.get()?;
        diesel::delete(trace_versions::table.filter(trace_versions::id.eq(self.id)))
            .execute(&mut conn)?;
        Ok(self)
    }
}

fn ensure_owner(version: &TraceVersion, user_id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    let trace = Trace::find_full_trace(version.trace_id, pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    Ok(())
}

#[debug_handler]
pub async fn post_trace_draft_version_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
) -> Result<Json<TraceVersion>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    Ok(Json(TraceVersion::get_or_create_draft_for_trace(
        trace_id, user_id, &pool,
    )?))
}

#[debug_handler]
pub async fn get_trace_version_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<TraceVersion>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let version = TraceVersion::find(id, &pool)?;
    ensure_owner(&version, user_id, &pool)?;
    Ok(Json(version))
}

#[debug_handler]
pub async fn put_trace_version_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTraceVersionDto>,
) -> Result<Json<TraceVersion>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let version = TraceVersion::find(id, &pool)?;
    ensure_owner(&version, user_id, &pool)?;
    let updated = version.update_from_payload(payload, &pool)?;
    if updated.status == TraceVersionStatus::Finalized {
        let trace = Trace::find_full_trace(updated.trace_id, &pool)?;
        crate::entities_v2::trace::routes::enqueue_trace_finalized_side_effects(&trace, &pool);
    }
    Ok(Json(updated))
}

#[debug_handler]
pub async fn post_trace_version_asset_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<TraceVersionAssetUploadResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut version = TraceVersion::find(id, &pool)?;
    ensure_owner(&version, user_id, &pool)?;
    if version.status != TraceVersionStatus::Draft {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Only draft trace versions can receive uploaded assets".to_string(),
        ));
    }

    let AssetUploadResponse {
        asset,
        signed_url,
        expires_at,
    } = upload_image_asset_for_user_from_multipart(user_id, &pool, multipart).await?;

    version.content_image_asset_id = Some(asset.id);
    let trace_version = version.save_draft(&pool)?;

    Ok(Json(TraceVersionAssetUploadResponse {
        trace_version,
        asset,
        signed_url,
        expires_at,
    }))
}

#[debug_handler]
pub async fn delete_trace_version_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<TraceVersion>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let version = TraceVersion::find(id, &pool)?;
    ensure_owner(&version, user_id, &pool)?;
    Ok(Json(version.delete_draft(&pool)?))
}
