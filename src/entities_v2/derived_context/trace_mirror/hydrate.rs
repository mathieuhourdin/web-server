use chrono::NaiveDateTime;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::Text;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::schema::trace_mirrors;
use crate::entities_v2::landmark::{Landmark, LandmarkType};
use crate::entities_v2::reference::Reference;

use super::model::{TraceMirror, TraceMirrorType};

type TraceMirrorTuple = (
    Uuid,
    String,
    String,
    String,
    String,
    String,
    Uuid,
    Uuid,
    Uuid,
    Option<Uuid>,
    NaiveDateTime,
    NaiveDateTime,
);

fn parse_json_string_array(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value).unwrap_or_default()
}

fn tuple_to_trace_mirror(row: TraceMirrorTuple) -> TraceMirror {
    let (
        id,
        title,
        subtitle,
        content,
        trace_mirror_type_raw,
        tags_json,
        trace_id,
        landscape_analysis_id,
        user_id,
        primary_landmark_id,
        created_at,
        updated_at,
    ) = row;

    TraceMirror {
        id,
        title,
        subtitle,
        content,
        trace_mirror_type: TraceMirrorType::from_db(&trace_mirror_type_raw),
        tags: parse_json_string_array(&tags_json),
        trace_id,
        landscape_analysis_id,
        user_id,
        primary_resource_id: primary_landmark_id,
        created_at,
        updated_at,
    }
}

impl TraceMirror {
    pub fn find_full_trace_mirror(id: Uuid, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let row = trace_mirrors::table
            .filter(trace_mirrors::id.eq(id))
            .select((
                trace_mirrors::id,
                trace_mirrors::title,
                trace_mirrors::subtitle,
                trace_mirrors::content,
                trace_mirrors::trace_mirror_type,
                sql::<Text>("tags::text"),
                trace_mirrors::trace_id,
                trace_mirrors::landscape_analysis_id,
                trace_mirrors::user_id,
                trace_mirrors::primary_landmark_id,
                trace_mirrors::created_at,
                trace_mirrors::updated_at,
            ))
            .first::<TraceMirrorTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_trace_mirror).ok_or_else(|| {
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
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = trace_mirrors::table
            .filter(trace_mirrors::landscape_analysis_id.eq(landscape_analysis_id))
            .select((
                trace_mirrors::id,
                trace_mirrors::title,
                trace_mirrors::subtitle,
                trace_mirrors::content,
                trace_mirrors::trace_mirror_type,
                sql::<Text>("tags::text"),
                trace_mirrors::trace_id,
                trace_mirrors::landscape_analysis_id,
                trace_mirrors::user_id,
                trace_mirrors::primary_landmark_id,
                trace_mirrors::created_at,
                trace_mirrors::updated_at,
            ))
            .order(trace_mirrors::created_at.asc())
            .load::<TraceMirrorTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_trace_mirror).collect())
    }

    pub fn find_by_trace(trace_id: Uuid, pool: &DbPool) -> Result<Vec<TraceMirror>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = trace_mirrors::table
            .filter(trace_mirrors::trace_id.eq(trace_id))
            .select((
                trace_mirrors::id,
                trace_mirrors::title,
                trace_mirrors::subtitle,
                trace_mirrors::content,
                trace_mirrors::trace_mirror_type,
                sql::<Text>("tags::text"),
                trace_mirrors::trace_id,
                trace_mirrors::landscape_analysis_id,
                trace_mirrors::user_id,
                trace_mirrors::primary_landmark_id,
                trace_mirrors::created_at,
                trace_mirrors::updated_at,
            ))
            .order(trace_mirrors::created_at.asc())
            .load::<TraceMirrorTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_trace_mirror).collect())
    }

    pub fn find_by_user(user_id: Uuid, pool: &DbPool) -> Result<Vec<TraceMirror>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = trace_mirrors::table
            .filter(trace_mirrors::user_id.eq(user_id))
            .select((
                trace_mirrors::id,
                trace_mirrors::title,
                trace_mirrors::subtitle,
                trace_mirrors::content,
                trace_mirrors::trace_mirror_type,
                sql::<Text>("tags::text"),
                trace_mirrors::trace_id,
                trace_mirrors::landscape_analysis_id,
                trace_mirrors::user_id,
                trace_mirrors::primary_landmark_id,
                trace_mirrors::created_at,
                trace_mirrors::updated_at,
            ))
            .order(trace_mirrors::created_at.desc())
            .load::<TraceMirrorTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_trace_mirror).collect())
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
