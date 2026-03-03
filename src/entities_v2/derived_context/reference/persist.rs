use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Bool, Int4, Nullable, Text, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::schema::references;

use super::model::{NewReference, Reference};

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

impl NewReference {
    pub fn create(self, pool: &DbPool) -> Result<Reference, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let context_tags_json =
            serde_json::to_string(&self.context_tags).unwrap_or_else(|_| "[]".to_string());
        let reference_variants_json =
            serde_json::to_string(&self.reference_variants).unwrap_or_else(|_| "[]".to_string());

        let id = sql_query(
            r#"
            INSERT INTO "references" (
                tag_id, trace_mirror_id, landmark_id, landscape_analysis_id, user_id,
                mention, reference_type, context_tags, reference_variants, parent_reference_id, is_user_specific
            )
            VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, CAST($8 AS jsonb), CAST($9 AS jsonb), $10, $11
            )
            RETURNING id
            "#,
        )
        .bind::<Int4, _>(self.tag_id)
        .bind::<SqlUuid, _>(self.trace_mirror_id)
        .bind::<Nullable<SqlUuid>, _>(self.landmark_id)
        .bind::<SqlUuid, _>(self.landscape_analysis_id)
        .bind::<SqlUuid, _>(self.user_id)
        .bind::<Text, _>(self.mention)
        .bind::<Text, _>(self.reference_type.to_db())
        .bind::<Text, _>(context_tags_json)
        .bind::<Text, _>(reference_variants_json)
        .bind::<Nullable<SqlUuid>, _>(self.parent_reference_id)
        .bind::<Bool, _>(self.is_user_specific)
        .get_result::<IdRow>(&mut conn)?
        .id;

        Reference::find(id, pool)
    }
}

impl Reference {
    pub fn delete(self, pool: &DbPool) -> Result<Reference, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let deleted = diesel::delete(references::table.filter(references::id.eq(self.id)))
            .execute(&mut conn)?;
        if deleted == 0 {
            return Err(PpdcError::new(
                404,
                ErrorType::ApiError,
                "Reference not found".to_string(),
            ));
        }
        Ok(self)
    }

    pub fn delete_for_landscape_analysis(
        landscape_analysis_id: Uuid,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::delete(
            references::table.filter(references::landscape_analysis_id.eq(landscape_analysis_id)),
        )
        .execute(&mut conn)?;
        Ok(())
    }
}
