use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    resource_relation::NewResourceRelation,
};
use crate::schema::resource_relations;

use super::model::{NewReference, Reference, ReferenceRelationComment, REFERENCE_RELATION_TYPE};

fn to_relation_comment_json(payload: &ReferenceRelationComment) -> Result<String, PpdcError> {
    serde_json::to_string(payload).map_err(|err| {
        PpdcError::new(
            500,
            ErrorType::InternalError,
            format!("Failed to serialize reference relation_comment: {}", err),
        )
    })
}

impl NewReference {
    pub fn create(self, pool: &DbPool) -> Result<Reference, PpdcError> {
        let landmark_id = self.landmark_id.ok_or_else(|| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                "landmark_id is required to persist a reference relation".to_string(),
            )
        })?;
        let user_id = self.user_id;

        let relation_comment = to_relation_comment_json(&ReferenceRelationComment::from(&self))?;

        let mut new_relation = NewResourceRelation::new(self.trace_mirror_id, landmark_id);
        new_relation.relation_type = Some(REFERENCE_RELATION_TYPE.to_string());
        new_relation.user_id = Some(user_id);
        new_relation.relation_comment = relation_comment.clone();
        new_relation.create(pool)?;

        // `ResourceRelation` fields are private, so we fetch the inserted row id.
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let id = resource_relations::table
            .filter(resource_relations::origin_resource_id.eq(self.trace_mirror_id))
            .filter(resource_relations::target_resource_id.eq(landmark_id))
            .filter(resource_relations::relation_type.eq(REFERENCE_RELATION_TYPE))
            .filter(resource_relations::user_id.eq(user_id))
            .filter(resource_relations::relation_comment.eq(relation_comment))
            .order(resource_relations::created_at.desc())
            .select(resource_relations::id)
            .first::<Uuid>(&mut conn)?;

        Reference::find(id, pool)
    }
}

impl Reference {
    pub fn delete(self, pool: &DbPool) -> Result<Reference, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        diesel::delete(resource_relations::table)
            .filter(resource_relations::id.eq(self.id))
            .execute(&mut conn)?;
        Ok(self)
    }

    pub fn delete_for_landscape_analysis(
        landscape_analysis_id: Uuid,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        let references = Reference::find_for_landscape_analysis(landscape_analysis_id, pool)?;
        for reference in references {
            reference.delete(pool)?;
        }
        Ok(())
    }
}
