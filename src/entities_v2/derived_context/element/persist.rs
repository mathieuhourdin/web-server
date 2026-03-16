use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Uuid as SqlUuid;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::{element_landmarks, elements};

use super::model::{Element, NewElement};

impl Element {
    pub fn update(self, pool: &DbPool) -> Result<Element, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        diesel::update(elements::table.filter(elements::id.eq(self.id)))
            .set((
                elements::title.eq(self.title),
                elements::subtitle.eq(self.subtitle),
                elements::content.eq(self.content),
                elements::extended_content.eq(self.extended_content),
                elements::verb.eq(self.verb),
                elements::element_type.eq(self.element_type.to_db()),
                elements::element_subtype.eq(self.element_subtype.to_db()),
                elements::interaction_date.eq(self.interaction_date),
                elements::user_id.eq(self.user_id),
                elements::analysis_id.eq(self.analysis_id),
                elements::trace_id.eq(self.trace_id),
                elements::trace_mirror_id.eq(self.trace_mirror_id),
            ))
            .execute(&mut conn)?;

        Element::find_full(self.id, pool)
    }
}

impl NewElement {
    pub fn create(self, pool: &DbPool) -> Result<Element, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let landmark_id = self.landmark_id;
        let id = conn.transaction::<Uuid, diesel::result::Error, _>(|conn| {
            let id: Uuid = diesel::insert_into(elements::table)
                .values((
                    elements::title.eq(&self.title),
                    elements::subtitle.eq(&self.subtitle),
                    elements::content.eq(&self.content),
                    elements::extended_content.eq::<Option<String>>(None),
                    elements::verb.eq(&self.verb),
                    elements::element_type.eq(self.element_type.to_db()),
                    elements::element_subtype.eq(self.element_subtype.to_db()),
                    elements::interaction_date.eq(self.interaction_date),
                    elements::user_id.eq(self.user_id),
                    elements::analysis_id.eq(self.analysis_id),
                    elements::trace_id.eq(self.trace_id),
                    elements::trace_mirror_id.eq(self.trace_mirror_id),
                ))
                .returning(elements::id)
                .get_result(conn)?;

            if let Some(lid) = landmark_id {
                let inserted = diesel::insert_into(element_landmarks::table)
                    .values((
                        element_landmarks::element_id.eq(id),
                        element_landmarks::landmark_id.eq(lid),
                    ))
                    .on_conflict((
                        element_landmarks::element_id,
                        element_landmarks::landmark_id,
                    ))
                    .do_nothing()
                    .execute(conn)?;
                if inserted > 0 {
                    refresh_landmark_related_elements_stats(lid, conn)?;
                }
            }

            Ok(id)
        })?;

        Element::find_full(id, pool)
    }
}

pub fn link_to_landmark(
    element_id: Uuid,
    landmark_id: Uuid,
    _user_id: Uuid,
    pool: &DbPool,
) -> Result<Uuid, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    conn.transaction::<(), diesel::result::Error, _>(|conn| {
        let inserted = diesel::insert_into(element_landmarks::table)
            .values((
                element_landmarks::element_id.eq(element_id),
                element_landmarks::landmark_id.eq(landmark_id),
            ))
            .on_conflict((
                element_landmarks::element_id,
                element_landmarks::landmark_id,
            ))
            .do_nothing()
            .execute(conn)?;
        if inserted > 0 {
            refresh_landmark_related_elements_stats(landmark_id, conn)?;
        }
        Ok(())
    })?;
    Ok(landmark_id)
}

fn refresh_landmark_related_elements_stats(
    landmark_id: Uuid,
    conn: &mut diesel::PgConnection,
) -> Result<(), diesel::result::Error> {
    sql_query(
        r#"
        UPDATE landmarks
        SET related_elements_count = stats.related_elements_count,
            last_related_element_at = stats.last_related_element_at,
            updated_at = NOW()
        FROM (
            SELECT
                el.landmark_id,
                COUNT(*)::INT AS related_elements_count,
                MAX(e.interaction_date) AS last_related_element_at
            FROM element_landmarks el
            INNER JOIN elements e ON e.id = el.element_id
            WHERE el.landmark_id = $1
            GROUP BY el.landmark_id
        ) AS stats
        WHERE landmarks.id = stats.landmark_id
        "#,
    )
    .bind::<SqlUuid, _>(landmark_id)
    .execute(conn)?;

    sql_query(
        r#"
        UPDATE landmarks
        SET related_elements_count = 0,
            last_related_element_at = NULL,
            updated_at = NOW()
        WHERE id = $1
          AND NOT EXISTS (
              SELECT 1
              FROM element_landmarks el
              WHERE el.landmark_id = $1
          )
        "#,
    )
    .bind::<SqlUuid, _>(landmark_id)
    .execute(conn)?;

    Ok(())
}
