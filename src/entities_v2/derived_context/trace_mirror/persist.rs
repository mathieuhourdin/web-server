use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::schema::{landscape_analyses, trace_mirrors};

use super::model::{NewTraceMirror, TraceMirror};

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

impl TraceMirror {
    pub fn update(self, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let tags_json = serde_json::to_string(&self.tags).unwrap_or_else(|_| "[]".to_string());
        sql_query(
            r#"
            UPDATE trace_mirrors
            SET title = $1,
                subtitle = $2,
                content = $3,
                trace_mirror_type = $4,
                tags = CAST($5 AS jsonb),
                trace_id = $6,
                landscape_analysis_id = $7,
                user_id = $8,
                primary_landmark_id = $9
            WHERE id = $10
            "#,
        )
        .bind::<Text, _>(self.title)
        .bind::<Text, _>(self.subtitle)
        .bind::<Text, _>(self.content)
        .bind::<Text, _>(self.trace_mirror_type.to_db())
        .bind::<Text, _>(tags_json)
        .bind::<SqlUuid, _>(self.trace_id)
        .bind::<SqlUuid, _>(self.landscape_analysis_id)
        .bind::<SqlUuid, _>(self.user_id)
        .bind::<Nullable<SqlUuid>, _>(self.primary_resource_id)
        .bind::<SqlUuid, _>(self.id)
        .execute(&mut conn)?;

        TraceMirror::find_full_trace_mirror(self.id, pool)
    }
}

impl NewTraceMirror {
    pub fn create(self, pool: &DbPool) -> Result<TraceMirror, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let tags_json = serde_json::to_string(&self.tags).unwrap_or_else(|_| "[]".to_string());
        let id = sql_query(
            r#"
            INSERT INTO trace_mirrors (
                title, subtitle, content, trace_mirror_type, tags,
                trace_id, landscape_analysis_id, user_id, primary_landmark_id
            )
            VALUES ($1, $2, $3, $4, CAST($5 AS jsonb), $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind::<Text, _>(self.title)
        .bind::<Text, _>(self.subtitle)
        .bind::<Text, _>(self.content)
        .bind::<Text, _>(self.trace_mirror_type.to_db())
        .bind::<Text, _>(tags_json)
        .bind::<SqlUuid, _>(self.trace_id)
        .bind::<SqlUuid, _>(self.landscape_analysis_id)
        .bind::<SqlUuid, _>(self.user_id)
        .bind::<Nullable<SqlUuid>, _>(self.primary_resource_id)
        .get_result::<IdRow>(&mut conn)?
        .id;

        diesel::update(
            landscape_analyses::table.filter(landscape_analyses::id.eq(self.landscape_analysis_id)),
        )
        .set(landscape_analyses::trace_mirror_id.eq(Some(id)))
        .execute(&mut conn)?;

        TraceMirror::find_full_trace_mirror(id, pool)
    }
}

pub fn link_to_primary_resource(
    trace_mirror_id: Uuid,
    primary_resource_id: Uuid,
    _user_id: Uuid,
    pool: &DbPool,
) -> Result<Uuid, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    diesel::update(trace_mirrors::table.filter(trace_mirrors::id.eq(trace_mirror_id)))
        .set(trace_mirrors::primary_landmark_id.eq(Some(primary_resource_id)))
        .execute(&mut conn)?;
    Ok(primary_resource_id)
}
