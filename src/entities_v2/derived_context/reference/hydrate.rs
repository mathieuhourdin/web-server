use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::schema::resource_relations;

use super::model::{Reference, ReferenceRelationComment, REFERENCE_RELATION_TYPE};

#[derive(Queryable, Selectable)]
#[diesel(table_name = resource_relations)]
struct ReferenceRelationRow {
    id: Uuid,
    origin_resource_id: Uuid,
    target_resource_id: Uuid,
    relation_comment: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    user_id: Uuid,
    relation_type: String,
}

fn parse_relation_comment(comment: &str) -> Result<ReferenceRelationComment, PpdcError> {
    serde_json::from_str(comment).map_err(|err| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            format!("Failed to parse reference relation_comment: {}", err),
        )
    })
}

fn row_to_reference(row: ReferenceRelationRow) -> Result<Reference, PpdcError> {
    let payload = parse_relation_comment(&row.relation_comment)?;
    let _ = row.relation_type;

    Ok(Reference {
        id: row.id,
        tag_id: payload.tag_id,
        trace_mirror_id: row.origin_resource_id,
        landmark_id: Some(row.target_resource_id),
        landscape_analysis_id: payload.landscape_analysis_id,
        user_id: row.user_id,
        mention: payload.mention,
        reference_type: payload.reference_type,
        context_tags: payload.context_tags,
        reference_variants: payload.reference_variants,
        parent_reference_id: payload.parent_reference_id,
        is_user_specific: payload.is_user_specific,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

impl Reference {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Reference, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let row = resource_relations::table
            .filter(resource_relations::id.eq(id))
            .filter(resource_relations::relation_type.eq(REFERENCE_RELATION_TYPE))
            .select(ReferenceRelationRow::as_select())
            .first::<ReferenceRelationRow>(&mut conn)?;

        row_to_reference(row)
    }

    pub fn find_for_trace_mirror(
        trace_mirror_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Reference>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let rows = resource_relations::table
            .filter(resource_relations::origin_resource_id.eq(trace_mirror_id))
            .filter(resource_relations::relation_type.eq(REFERENCE_RELATION_TYPE))
            .order(resource_relations::created_at.asc())
            .select(ReferenceRelationRow::as_select())
            .load::<ReferenceRelationRow>(&mut conn)?;

        rows.into_iter()
            .map(row_to_reference)
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn find_for_landmark(
        landmark_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Reference>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let rows = resource_relations::table
            .filter(resource_relations::target_resource_id.eq(landmark_id))
            .filter(resource_relations::relation_type.eq(REFERENCE_RELATION_TYPE))
            .order(resource_relations::created_at.asc())
            .select(ReferenceRelationRow::as_select())
            .load::<ReferenceRelationRow>(&mut conn)?;

        rows.into_iter()
            .map(row_to_reference)
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn find_for_landscape_analysis(
        landscape_analysis_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Reference>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let rows = resource_relations::table
            .filter(resource_relations::relation_type.eq(REFERENCE_RELATION_TYPE))
            .order(resource_relations::created_at.asc())
            .select(ReferenceRelationRow::as_select())
            .load::<ReferenceRelationRow>(&mut conn)?;

        rows.into_iter()
            .map(row_to_reference)
            .filter_map(|reference| match reference {
                Ok(reference) if reference.landscape_analysis_id == landscape_analysis_id => {
                    Some(Ok(reference))
                }
                Ok(_) => None,
                Err(err) => Some(Err(err)),
            })
            .collect::<Result<Vec<_>, _>>()
    }
}
