use diesel::prelude::*;
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
        let id: Uuid = diesel::insert_into(elements::table)
            .values((
                elements::title.eq(self.title),
                elements::subtitle.eq(self.subtitle),
                elements::content.eq(self.content),
                elements::extended_content.eq::<Option<String>>(None),
                elements::verb.eq(self.verb),
                elements::element_type.eq(self.element_type.to_db()),
                elements::element_subtype.eq(self.element_subtype.to_db()),
                elements::interaction_date.eq(self.interaction_date),
                elements::user_id.eq(self.user_id),
                elements::analysis_id.eq(self.analysis_id),
                elements::trace_id.eq(self.trace_id),
                elements::trace_mirror_id.eq(self.trace_mirror_id),
            ))
            .returning(elements::id)
            .get_result(&mut conn)?;

        if let Some(lid) = landmark_id {
            diesel::insert_into(element_landmarks::table)
                .values((
                    element_landmarks::element_id.eq(id),
                    element_landmarks::landmark_id.eq(lid),
                ))
                .on_conflict((
                    element_landmarks::element_id,
                    element_landmarks::landmark_id,
                ))
                .do_nothing()
                .execute(&mut conn)?;
        }

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
    diesel::insert_into(element_landmarks::table)
        .values((
            element_landmarks::element_id.eq(element_id),
            element_landmarks::landmark_id.eq(landmark_id),
        ))
        .on_conflict((
            element_landmarks::element_id,
            element_landmarks::landmark_id,
        ))
        .do_nothing()
        .execute(&mut conn)?;
    Ok(landmark_id)
}
