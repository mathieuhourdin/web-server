use chrono::NaiveDateTime;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::Text;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::schema::references;

use super::model::{Reference, ReferenceType};

type ReferenceTuple = (
    Uuid,
    i32,
    Uuid,
    Option<Uuid>,
    Uuid,
    Uuid,
    String,
    String,
    String,
    String,
    Option<Uuid>,
    bool,
    NaiveDateTime,
    NaiveDateTime,
);

fn parse_json_string_array(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value).unwrap_or_default()
}

fn tuple_to_reference(row: ReferenceTuple) -> Reference {
    let (
        id,
        tag_id,
        trace_mirror_id,
        landmark_id,
        landscape_analysis_id,
        user_id,
        mention,
        reference_type_raw,
        context_tags_json,
        reference_variants_json,
        parent_reference_id,
        is_user_specific,
        created_at,
        updated_at,
    ) = row;

    Reference {
        id,
        tag_id,
        trace_mirror_id,
        landmark_id,
        landscape_analysis_id,
        user_id,
        mention,
        reference_type: ReferenceType::from_db(&reference_type_raw),
        context_tags: parse_json_string_array(&context_tags_json),
        reference_variants: parse_json_string_array(&reference_variants_json),
        parent_reference_id,
        is_user_specific,
        created_at,
        updated_at,
    }
}

impl Reference {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Reference, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let row = references::table
            .filter(references::id.eq(id))
            .select((
                references::id,
                references::tag_id,
                references::trace_mirror_id,
                references::landmark_id,
                references::landscape_analysis_id,
                references::user_id,
                references::mention,
                references::reference_type,
                sql::<Text>("context_tags::text"),
                sql::<Text>("reference_variants::text"),
                references::parent_reference_id,
                references::is_user_specific,
                references::created_at,
                references::updated_at,
            ))
            .first::<ReferenceTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_reference).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Reference not found".to_string(),
            )
        })
    }

    pub fn find_for_trace_mirror(
        trace_mirror_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Reference>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = references::table
            .filter(references::trace_mirror_id.eq(trace_mirror_id))
            .select((
                references::id,
                references::tag_id,
                references::trace_mirror_id,
                references::landmark_id,
                references::landscape_analysis_id,
                references::user_id,
                references::mention,
                references::reference_type,
                sql::<Text>("context_tags::text"),
                sql::<Text>("reference_variants::text"),
                references::parent_reference_id,
                references::is_user_specific,
                references::created_at,
                references::updated_at,
            ))
            .order(references::created_at.asc())
            .load::<ReferenceTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_reference).collect())
    }

    pub fn find_for_landmark(
        landmark_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Reference>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = references::table
            .filter(references::landmark_id.eq(landmark_id))
            .select((
                references::id,
                references::tag_id,
                references::trace_mirror_id,
                references::landmark_id,
                references::landscape_analysis_id,
                references::user_id,
                references::mention,
                references::reference_type,
                sql::<Text>("context_tags::text"),
                sql::<Text>("reference_variants::text"),
                references::parent_reference_id,
                references::is_user_specific,
                references::created_at,
                references::updated_at,
            ))
            .order(references::created_at.asc())
            .load::<ReferenceTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_reference).collect())
    }

    pub fn find_for_landscape_analysis(
        landscape_analysis_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Reference>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = references::table
            .filter(references::landscape_analysis_id.eq(landscape_analysis_id))
            .select((
                references::id,
                references::tag_id,
                references::trace_mirror_id,
                references::landmark_id,
                references::landscape_analysis_id,
                references::user_id,
                references::mention,
                references::reference_type,
                sql::<Text>("context_tags::text"),
                sql::<Text>("reference_variants::text"),
                references::parent_reference_id,
                references::is_user_specific,
                references::created_at,
                references::updated_at,
            ))
            .order(references::created_at.asc())
            .load::<ReferenceTuple>(&mut conn)?;
        Ok(rows.into_iter().map(tuple_to_reference).collect())
    }
}
