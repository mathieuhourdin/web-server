use crate::entities_v2::platform_infra::asset::Asset;
use axum::{extract::Path, Extension, Json};
use chrono::NaiveDateTime;
use diesel::{
    prelude::*,
    sql_types::{Float8, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::model::{Trace, TraceStatus};
use crate::entities_v2::records::trace_source_asset::TraceSourceAsset;
use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        session::Session,
    },
};

#[derive(Debug, Serialize, Clone)]
pub struct TranscriptionJob {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub source_asset_ids: serde_json::Value,
    pub status: String,
    pub pipeline: String,
    pub canonical_text: Option<String>,
    pub challenges: serde_json::Value,
    pub estimated_cost_usd: Option<f64>,
    pub error_message: Option<String>,
    pub created_at: NaiveDateTime,
    pub started_at: Option<NaiveDateTime>,
    pub completed_at: Option<NaiveDateTime>,
    pub confirmed_at: Option<NaiveDateTime>,
}

#[derive(QueryableByName)]
struct JobDbRow {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=SqlUuid)]
    trace_id: Uuid,
    #[diesel(sql_type=Text)]
    source_asset_ids: String,
    #[diesel(sql_type=Text)]
    status: String,
    #[diesel(sql_type=Text)]
    pipeline: String,
    #[diesel(sql_type=Nullable<Text>)]
    canonical_text: Option<String>,
    #[diesel(sql_type=Text)]
    challenges: String,
    #[diesel(sql_type=Nullable<Float8>)]
    estimated_cost_usd: Option<f64>,
    #[diesel(sql_type=Nullable<Text>)]
    error_message: Option<String>,
    #[diesel(sql_type=Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type=Nullable<Timestamp>)]
    started_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    completed_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    confirmed_at: Option<NaiveDateTime>,
}

fn get_job(id: Uuid, pool: &DbPool) -> Result<TranscriptionJob, PpdcError> {
    let mut c = pool.get()?;
    diesel::sql_query("SELECT id,trace_id,source_asset_ids::text,status,pipeline,canonical_text,challenges::text,estimated_cost_usd,error_message,created_at,started_at,completed_at,confirmed_at FROM transcription_jobs WHERE id=$1")
        .bind::<SqlUuid,_>(id).get_result::<JobDbRow>(&mut c)
        .map(|r| TranscriptionJob {
            id: r.id,
            trace_id: r.trace_id,
            source_asset_ids: serde_json::from_str(&r.source_asset_ids).unwrap_or(json!([])),
            status: r.status,
            pipeline: r.pipeline,
            canonical_text: r.canonical_text,
            challenges: serde_json::from_str(&r.challenges).unwrap_or(json!([])),
            estimated_cost_usd: r.estimated_cost_usd,
            error_message: r.error_message,
            created_at: r.created_at,
            started_at: r.started_at,
            completed_at: r.completed_at,
            confirmed_at: r.confirmed_at,
        })
        .map_err(|e| match e {
            diesel::result::Error::NotFound => PpdcError::new(
                404,
                ErrorType::ApiError,
                "Transcription job not found".into(),
            ),
            other => other.into(),
        })
}

#[derive(Deserialize)]
pub struct StartTranscriptionJob {
    pub pipeline: Option<String>,
}
#[derive(Deserialize)]
pub struct ConfirmTranscriptionJob {
    pub expected_version_integer: i32,
    pub content: String,
    pub title: Option<String>,
    pub interaction_date: Option<NaiveDateTime>,
}

pub async fn post_transcription_job_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(trace_id): Path<Uuid>,
    Json(body): Json<StartTranscriptionJob>,
) -> Result<Json<TranscriptionJob>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    if trace.status != TraceStatus::Draft {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Transcription is only available for draft traces".into(),
        ));
    }
    let assets = TraceSourceAsset::find_for_trace(trace_id, &pool)?;
    if assets.is_empty() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "Upload at least one source image first".into(),
        ));
    }
    let ids = serde_json::Value::Array(assets.into_iter().map(|a| json!(a.asset_id)).collect());
    let pipeline = body.pipeline.unwrap_or_else(|| "gpt4_google_luna".into());
    let mut c = pool.get()?;
    #[derive(QueryableByName)]
    struct IdRow {
        #[diesel(sql_type = SqlUuid)]
        id: Uuid,
    }
    let id: Uuid = diesel::sql_query("INSERT INTO transcription_jobs (trace_id,source_asset_ids,status,pipeline) VALUES ($1,CAST($2 AS jsonb),'QUEUED',$3) RETURNING id").bind::<SqlUuid,_>(trace_id).bind::<Text,_>(ids.to_string()).bind::<Text,_>(&pipeline).get_result::<IdRow>(&mut c)?.id;
    let worker_pool = pool.clone();
    tokio::spawn(async move {
        process_job(id, worker_pool).await;
    });
    Ok(Json(get_job(id, &pool)?))
}

async fn process_job(job_id: Uuid, pool: DbPool) {
    let result = async {
        set_job_status(job_id, "RUNNING", &pool)?;
        let job = get_job(job_id, &pool)?;
        let ids: Vec<Uuid> = serde_json::from_value(job.source_asset_ids.clone()).map_err(|e| PpdcError::new(500, ErrorType::InternalError, e.to_string()))?;
        let assets = Asset::find_by_ids(&ids, &pool)?;
        let mut images = Vec::new();
        for id in ids { let asset = assets.get(&id).ok_or_else(|| PpdcError::new(500, ErrorType::InternalError, "Source asset missing".into()))?; images.push(asset.signed_read_url(3600).await?.0); }
        let key = crate::environment::get_openai_api_key();
        let base = crate::environment::get_openai_api_base_url().trim_end_matches('/').to_string();
        let url = if base.ends_with("/v1") { format!("{base}/responses") } else { format!("{base}/v1/responses") };
        let content: Vec<serde_json::Value> = std::iter::once(json!({"type":"input_text","text":"Transcribe these handwritten diary pages faithfully. Return only the transcription."})).chain(images.into_iter().map(|u| json!({"type":"input_image","image_url":u,"detail":"high"}))).collect();
        let response: serde_json::Value = reqwest::Client::new().post(url).bearer_auth(key).json(&json!({"model":"gpt-4.1-mini","input":[{"role":"user","content":content}],"store":false,"max_output_tokens":12000})).send().await.map_err(|e| PpdcError::new(502, ErrorType::InternalError, e.to_string()))?.json().await.map_err(|e| PpdcError::new(502, ErrorType::InternalError, e.to_string()))?;
        let text = response.get("output_text").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if text.is_empty() { return Err(PpdcError::new(502, ErrorType::InternalError, "OpenAI returned no transcription".into())); }
        update_job_result(job_id, &text, response.get("usage").cloned().unwrap_or(json!({})), &pool)?;
        Ok::<(), PpdcError>(())
    }.await;
    if let Err(error) = result {
        let _ = fail_job(job_id, &error.to_string(), &pool);
    }
}

fn set_job_status(id: Uuid, status: &str, pool: &DbPool) -> Result<(), PpdcError> {
    let mut c = pool.get()?;
    diesel::sql_query(
        "UPDATE transcription_jobs SET status=$2,started_at=COALESCE(started_at,NOW()) WHERE id=$1",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<Text, _>(status)
    .execute(&mut c)?;
    Ok(())
}
fn update_job_result(
    id: Uuid,
    text: &str,
    usage: serde_json::Value,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let mut c = pool.get()?;
    diesel::sql_query("UPDATE transcription_jobs SET status='REVIEW',canonical_text=$2,challenges='[]'::jsonb,completed_at=NOW() WHERE id=$1").bind::<SqlUuid,_>(id).bind::<Text,_>(text).execute(&mut c)?;
    let _ = usage;
    Ok(())
}
fn fail_job(id: Uuid, message: &str, pool: &DbPool) -> Result<(), PpdcError> {
    let mut c = pool.get()?;
    diesel::sql_query("UPDATE transcription_jobs SET status='FAILED',error_message=$2,completed_at=NOW() WHERE id=$1").bind::<SqlUuid,_>(id).bind::<Text,_>(message).execute(&mut c)?;
    Ok(())
}

pub async fn get_transcription_job_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((trace_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<TranscriptionJob>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let job = get_job(job_id, &pool)?;
    if job.trace_id != trace_id {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "Transcription job not found".into(),
        ));
    }
    Ok(Json(job))
}

pub async fn confirm_transcription_job_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((trace_id, job_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ConfirmTranscriptionJob>,
) -> Result<Json<Trace>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut trace = Trace::find_full_trace(trace_id, &pool)?;
    if trace.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let job = get_job(job_id, &pool)?;
    if job.trace_id != trace_id {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "Transcription job not found".into(),
        ));
    }
    if !matches!(job.status.as_str(), "REVIEW" | "QUEUED") {
        return Err(PpdcError::new(
            409,
            ErrorType::ApiError,
            "Transcription job is not reviewable".into(),
        ));
    }
    trace.content = body.content;
    if let Some(t) = body.title {
        trace.title = t;
    }
    if let Some(d) = body.interaction_date {
        trace.interaction_date = d;
    }
    let trace = trace.update_with_expected_version(body.expected_version_integer, &pool)?;
    let mut c = pool.get()?;
    diesel::sql_query("UPDATE transcription_jobs SET status='CONFIRMED',confirmed_at=NOW(),completed_at=COALESCE(completed_at,NOW()) WHERE id=$1").bind::<SqlUuid,_>(job_id).execute(&mut c)?;
    Ok(Json(trace))
}
