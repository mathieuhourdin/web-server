use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Bool, Int4, Nullable, Text, Timestamp, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};

use super::model::{Reference, ReferenceType};

#[derive(QueryableByName)]
struct ReferenceRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Int4)]
    tag_id: i32,
    #[diesel(sql_type = SqlUuid)]
    trace_mirror_id: Uuid,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    landmark_id: Option<Uuid>,
    #[diesel(sql_type = SqlUuid)]
    landscape_analysis_id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type = Text)]
    mention: String,
    #[diesel(sql_type = Text)]
    reference_type: String,
    #[diesel(sql_type = Text)]
    context_tags_json: String,
    #[diesel(sql_type = Text)]
    reference_variants_json: String,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    parent_reference_id: Option<Uuid>,
    #[diesel(sql_type = Bool)]
    is_user_specific: bool,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
}

fn parse_json_string_array(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value).unwrap_or_default()
}

fn row_to_reference(row: ReferenceRow) -> Reference {
    Reference {
        id: row.id,
        tag_id: row.tag_id,
        trace_mirror_id: row.trace_mirror_id,
        landmark_id: row.landmark_id,
        landscape_analysis_id: row.landscape_analysis_id,
        user_id: row.user_id,
        mention: row.mention,
        reference_type: ReferenceType::from_db(&row.reference_type),
        context_tags: parse_json_string_array(&row.context_tags_json),
        reference_variants: parse_json_string_array(&row.reference_variants_json),
        parent_reference_id: row.parent_reference_id,
        is_user_specific: row.is_user_specific,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn load_references_by_uuid(sql: &str, id: Uuid, pool: &DbPool) -> Result<Vec<Reference>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");

    let rows = sql_query(sql)
        .bind::<SqlUuid, _>(id)
        .load::<ReferenceRow>(&mut conn)?;
    Ok(rows.into_iter().map(row_to_reference).collect())
}

impl Reference {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Reference, PpdcError> {
        let mut refs = load_references_by_uuid(
            r#"
            SELECT id, tag_id, trace_mirror_id, landmark_id, landscape_analysis_id, user_id, mention,
                   reference_type, context_tags::text AS context_tags_json,
                   reference_variants::text AS reference_variants_json, parent_reference_id,
                   is_user_specific, created_at, updated_at
            FROM "references"
            WHERE id = $1
            "#,
            id,
            pool,
        )?;
        refs.pop().ok_or_else(|| {
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
        load_references_by_uuid(
            r#"
            SELECT id, tag_id, trace_mirror_id, landmark_id, landscape_analysis_id, user_id, mention,
                   reference_type, context_tags::text AS context_tags_json,
                   reference_variants::text AS reference_variants_json, parent_reference_id,
                   is_user_specific, created_at, updated_at
            FROM "references"
            WHERE trace_mirror_id = $1
            ORDER BY created_at ASC
            "#,
            trace_mirror_id,
            pool,
        )
    }

    pub fn find_for_landmark(
        landmark_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Reference>, PpdcError> {
        load_references_by_uuid(
            r#"
            SELECT id, tag_id, trace_mirror_id, landmark_id, landscape_analysis_id, user_id, mention,
                   reference_type, context_tags::text AS context_tags_json,
                   reference_variants::text AS reference_variants_json, parent_reference_id,
                   is_user_specific, created_at, updated_at
            FROM "references"
            WHERE landmark_id = $1
            ORDER BY created_at ASC
            "#,
            landmark_id,
            pool,
        )
    }

    pub fn find_for_landscape_analysis(
        landscape_analysis_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Reference>, PpdcError> {
        load_references_by_uuid(
            r#"
            SELECT id, tag_id, trace_mirror_id, landmark_id, landscape_analysis_id, user_id, mention,
                   reference_type, context_tags::text AS context_tags_json,
                   reference_variants::text AS reference_variants_json, parent_reference_id,
                   is_user_specific, created_at, updated_at
            FROM "references"
            WHERE landscape_analysis_id = $1
            ORDER BY created_at ASC
            "#,
            landscape_analysis_id,
            pool,
        )
    }
}
