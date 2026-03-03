use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::schema::{landmark_relations, landscape_landmarks, landmarks};

use super::model::{Landmark, NewLandmark};

impl Landmark {
    pub fn update(self, pool: &DbPool) -> Result<Landmark, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        diesel::update(landmarks::table.filter(landmarks::id.eq(self.id)))
            .set((
                landmarks::title.eq(self.title),
                landmarks::subtitle.eq(self.subtitle),
                landmarks::content.eq(self.content),
                landmarks::external_content_url.eq(self.external_content_url),
                landmarks::comment.eq(self.comment),
                landmarks::image_url.eq(self.image_url),
                landmarks::landmark_type.eq(self.landmark_type.to_code()),
                landmarks::maturing_state.eq(self.maturing_state.to_code()),
            ))
            .execute(&mut conn)?;

        Landmark::find(self.id, pool)
    }
}

impl NewLandmark {
    pub fn create(self, pool: &DbPool) -> Result<Landmark, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let analysis_id = self.analysis_id;
        let parent_id = self.parent_id;

        let id: Uuid = diesel::insert_into(landmarks::table)
            .values((
                landmarks::analysis_id.eq(self.analysis_id),
                landmarks::user_id.eq(self.user_id),
                landmarks::parent_id.eq(self.parent_id),
                landmarks::title.eq(self.title),
                landmarks::subtitle.eq(self.subtitle),
                landmarks::content.eq(self.content),
                landmarks::landmark_type.eq(self.landmark_type.to_code()),
                landmarks::maturing_state.eq(self.maturing_state.to_code()),
            ))
            .returning(landmarks::id)
            .get_result(&mut conn)?;

        diesel::insert_into(landscape_landmarks::table)
            .values((
                landscape_landmarks::landscape_analysis_id.eq(analysis_id),
                landscape_landmarks::landmark_id.eq(id),
                landscape_landmarks::relation_type.eq("OWNED_BY_ANALYSIS"),
            ))
            .on_conflict((
                landscape_landmarks::landscape_analysis_id,
                landscape_landmarks::landmark_id,
                landscape_landmarks::relation_type,
            ))
            .do_nothing()
            .execute(&mut conn)?;

        if let Some(parent_landmark_id) = parent_id {
            diesel::insert_into(landmark_relations::table)
                .values((
                    landmark_relations::origin_landmark_id.eq(id),
                    landmark_relations::target_landmark_id.eq(parent_landmark_id),
                    landmark_relations::relation_type.eq("CHILD_OF"),
                ))
                .on_conflict((
                    landmark_relations::origin_landmark_id,
                    landmark_relations::target_landmark_id,
                    landmark_relations::relation_type,
                ))
                .do_nothing()
                .execute(&mut conn)?;
        }

        Landmark::find(id, pool)
    }
}

pub fn create_copy_child_and_return(
    parent_landmark_id: Uuid,
    user_id: Uuid,
    analysis_id: Uuid,
    pool: &DbPool,
) -> Result<Landmark, PpdcError> {
    let parent_landmark = Landmark::find(parent_landmark_id, pool)?;
    let landmark = NewLandmark::new(
        parent_landmark.title,
        parent_landmark.subtitle,
        parent_landmark.content,
        parent_landmark.landmark_type,
        parent_landmark.maturing_state,
        analysis_id,
        user_id,
        Some(parent_landmark_id),
    );
    landmark.create(pool)
}
