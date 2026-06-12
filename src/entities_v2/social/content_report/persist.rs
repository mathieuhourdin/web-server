use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text, Timestamp, Uuid as SqlUuid};
use serde_json::Value;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};

use super::model::{
    ContentReport, ContentReportReason, ContentReportSourceKind, ContentReportStatus,
    NewContentReport,
};

#[derive(QueryableByName)]
struct ContentReportRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    reporter_user_id: Uuid,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    reported_message_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    reported_post_id: Option<Uuid>,
    #[diesel(sql_type = SqlUuid)]
    reported_user_id: Uuid,
    #[diesel(sql_type = Text)]
    reason: String,
    #[diesel(sql_type = Text)]
    reporter_comment: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    reviewed_by_user_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<Timestamp>)]
    reviewed_at: Option<NaiveDateTime>,
    #[diesel(sql_type = Text)]
    resolution_note: String,
    #[diesel(sql_type = Text)]
    snapshot_title: String,
    #[diesel(sql_type = Text)]
    snapshot_content: String,
    #[diesel(sql_type = Nullable<Text>)]
    snapshot_attachment_json: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    snapshot_metadata_json: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    snapshot_source_kind: Option<String>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    snapshot_source_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<Text>)]
    snapshot_context_json: Option<String>,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
}

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
}

fn parse_json(value: Option<String>) -> Option<Value> {
    value.and_then(|json| serde_json::from_str::<Value>(&json).ok())
}

impl From<ContentReportRow> for ContentReport {
    fn from(row: ContentReportRow) -> Self {
        Self {
            id: row.id,
            reporter_user_id: row.reporter_user_id,
            reported_message_id: row.reported_message_id,
            reported_post_id: row.reported_post_id,
            reported_user_id: row.reported_user_id,
            reason: ContentReportReason::from_db(&row.reason),
            reporter_comment: row.reporter_comment,
            status: ContentReportStatus::from_db(&row.status),
            reviewed_by_user_id: row.reviewed_by_user_id,
            reviewed_at: row.reviewed_at,
            resolution_note: row.resolution_note,
            snapshot_title: row.snapshot_title,
            snapshot_content: row.snapshot_content,
            snapshot_attachment_json: parse_json(row.snapshot_attachment_json),
            snapshot_metadata_json: parse_json(row.snapshot_metadata_json),
            snapshot_source_kind: row
                .snapshot_source_kind
                .as_deref()
                .map(ContentReportSourceKind::from_db),
            snapshot_source_id: row.snapshot_source_id,
            snapshot_context_json: parse_json(row.snapshot_context_json),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl ContentReport {
    pub fn find_paginated(
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<ContentReport>, i64), PpdcError> {
        let mut conn = pool.get()?;
        let total = sql_query("SELECT COUNT(*)::bigint AS count FROM content_reports")
            .get_result::<CountRow>(&mut conn)?
            .count;

        let rows = sql_query(
            "SELECT id, reporter_user_id, reported_message_id, reported_post_id, reported_user_id, reason, reporter_comment, status, reviewed_by_user_id, reviewed_at, resolution_note, snapshot_title, snapshot_content, snapshot_attachment_json::text AS snapshot_attachment_json, snapshot_metadata_json::text AS snapshot_metadata_json, snapshot_source_kind, snapshot_source_id, snapshot_context_json::text AS snapshot_context_json, created_at, updated_at
             FROM content_reports
             ORDER BY created_at DESC, id DESC
             OFFSET $1
             LIMIT $2",
        )
        .bind::<diesel::sql_types::BigInt, _>(offset)
        .bind::<diesel::sql_types::BigInt, _>(limit)
        .load::<ContentReportRow>(&mut conn)?;

        Ok((rows.into_iter().map(ContentReport::from).collect(), total))
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<ContentReport, PpdcError> {
        let mut conn = pool.get()?;
        let row = sql_query(
            "SELECT id, reporter_user_id, reported_message_id, reported_post_id, reported_user_id, reason, reporter_comment, status, reviewed_by_user_id, reviewed_at, resolution_note, snapshot_title, snapshot_content, snapshot_attachment_json::text AS snapshot_attachment_json, snapshot_metadata_json::text AS snapshot_metadata_json, snapshot_source_kind, snapshot_source_id, snapshot_context_json::text AS snapshot_context_json, created_at, updated_at
             FROM content_reports
             WHERE id = $1",
        )
        .bind::<SqlUuid, _>(id)
        .get_result::<ContentReportRow>(&mut conn)
        .optional()?;

        row.map(ContentReport::from).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Content report not found".to_string(),
            )
        })
    }
}

impl NewContentReport {
    pub fn create(self, pool: &DbPool) -> Result<ContentReport, PpdcError> {
        let mut conn = pool.get()?;
        let id = sql_query(
            "INSERT INTO content_reports (
                id,
                reporter_user_id,
                reported_message_id,
                reported_post_id,
                reported_user_id,
                reason,
                reporter_comment,
                status,
                reviewed_by_user_id,
                reviewed_at,
                resolution_note,
                snapshot_title,
                snapshot_content,
                snapshot_attachment_json,
                snapshot_metadata_json,
                snapshot_source_kind,
                snapshot_source_id,
                snapshot_context_json,
                created_at,
                updated_at
            ) VALUES (
                uuid_generate_v4(),
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                'OPEN',
                NULL,
                NULL,
                '',
                $7,
                $8,
                CAST($9 AS jsonb),
                CAST($10 AS jsonb),
                $11,
                $12,
                CAST($13 AS jsonb),
                NOW(),
                NOW()
            )
            RETURNING id",
        )
        .bind::<SqlUuid, _>(self.reporter_user_id)
        .bind::<Nullable<SqlUuid>, _>(self.reported_message_id)
        .bind::<Nullable<SqlUuid>, _>(self.reported_post_id)
        .bind::<SqlUuid, _>(self.reported_user_id)
        .bind::<Text, _>(self.reason.to_db())
        .bind::<Text, _>(self.reporter_comment)
        .bind::<Text, _>(self.snapshot_title)
        .bind::<Text, _>(self.snapshot_content)
        .bind::<Nullable<Text>, _>(
            self.snapshot_attachment_json
                .as_ref()
                .map(|value| value.to_string()),
        )
        .bind::<Nullable<Text>, _>(
            self.snapshot_metadata_json
                .as_ref()
                .map(|value| value.to_string()),
        )
        .bind::<Nullable<Text>, _>(
            self.snapshot_source_kind
                .map(|kind| kind.to_db().to_string()),
        )
        .bind::<Nullable<SqlUuid>, _>(self.snapshot_source_id)
        .bind::<Nullable<Text>, _>(
            self.snapshot_context_json
                .as_ref()
                .map(|value| value.to_string()),
        )
        .get_result::<IdRow>(&mut conn)?
        .id;

        ContentReport::find(id, pool)
    }
}
