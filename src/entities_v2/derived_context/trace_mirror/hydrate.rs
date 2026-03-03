use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text, Timestamp, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::entities_v2::landmark::{Landmark, LandmarkType};
use crate::entities_v2::reference::Reference;

use super::model::{TraceMirror, TraceMirrorType};

#[derive(QueryableByName)]
struct TraceMirrorRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    subtitle: String,
    #[diesel(sql_type = Text)]
    content: String,
    #[diesel(sql_type = Text)]
    trace_mirror_type: String,
    #[diesel(sql_type = Text)]
    tags_json: String,
    #[diesel(sql_type = SqlUuid)]
    trace_id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    landscape_analysis_id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    primary_landmark_id: Option<Uuid>,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
}

fn parse_json_string_array(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value).unwrap_or_default()
}

fn row_to_trace_mirror(row: TraceMirrorRow) -> TraceMirror {
    TraceMirror {
        id: row.id,
        title: row.title,
        subtitle: row.subtitle,
        content: row.content,
        trace_mirror_type: TraceMirrorType::from_db(&row.trace_mirror_type),
        tags: parse_json_string_array(&row.tags_json),
        trace_id: row.trace_id,
        landscape_analysis_id: row.landscape_analysis_id,
        user_id: row.user_id,
        primary_resource_id: row.primary_landmark_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn load_trace_mirrors_by_uuid(sql: &str, id: Uuid, pool: &DbPool) -> Result<Vec<TraceMirror>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let rows = sql_query(sql)
        .bind::<SqlUuid, _>(id)
        .load::<TraceMirrorRow>(&mut conn)?;
    Ok(rows.into_iter().map(row_to_trace_mirror).collect())
}

impl TraceMirror {
    pub fn find_full_trace_mirror(id: Uuid, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let mut result = load_trace_mirrors_by_uuid(
            r#"
            SELECT id, title, subtitle, content, trace_mirror_type, tags::text AS tags_json, trace_id,
                   landscape_analysis_id, user_id, primary_landmark_id,
                   created_at, updated_at
            FROM trace_mirrors
            WHERE id = $1
            "#,
            id,
            pool,
        )?;
        result.pop().ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Trace mirror not found".to_string(),
            )
        })
    }

    pub fn find_by_landscape_analysis(
        landscape_analysis_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<TraceMirror>, PpdcError> {
        load_trace_mirrors_by_uuid(
            r#"
            SELECT id, title, subtitle, content, trace_mirror_type, tags::text AS tags_json, trace_id,
                   landscape_analysis_id, user_id, primary_landmark_id,
                   created_at, updated_at
            FROM trace_mirrors
            WHERE landscape_analysis_id = $1
            ORDER BY created_at ASC
            "#,
            landscape_analysis_id,
            pool,
        )
    }

    pub fn find_by_trace(trace_id: Uuid, pool: &DbPool) -> Result<Vec<TraceMirror>, PpdcError> {
        load_trace_mirrors_by_uuid(
            r#"
            SELECT id, title, subtitle, content, trace_mirror_type, tags::text AS tags_json, trace_id,
                   landscape_analysis_id, user_id, primary_landmark_id,
                   created_at, updated_at
            FROM trace_mirrors
            WHERE trace_id = $1
            ORDER BY created_at ASC
            "#,
            trace_id,
            pool,
        )
    }

    pub fn find_by_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<TraceMirror>, PpdcError> {
        load_trace_mirrors_by_uuid(
            r#"
            SELECT id, title, subtitle, content, trace_mirror_type, tags::text AS tags_json, trace_id,
                   landscape_analysis_id, user_id, primary_landmark_id,
                   created_at, updated_at
            FROM trace_mirrors
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
            user_id,
            pool,
        )
    }

    pub fn get_high_level_projects_references(
        &self,
        pool: &DbPool,
    ) -> Result<Vec<Reference>, PpdcError> {
        let mut projects_references = Vec::new();
        let references = Reference::find_for_trace_mirror(self.id, pool)?;

        for reference in references {
            let Some(landmark_id) = reference.landmark_id else {
                continue;
            };
            let landmark = Landmark::find(landmark_id, pool)?;
            if landmark.landmark_type == LandmarkType::HighLevelProject {
                projects_references.push(reference);
            }
        }

        Ok(projects_references)
    }

    pub fn find_high_level_projects_references(
        trace_mirror_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Reference>, PpdcError> {
        let trace_mirror = TraceMirror::find_full_trace_mirror(trace_mirror_id, pool)?;
        trace_mirror.get_high_level_projects_references(pool)
    }
}
