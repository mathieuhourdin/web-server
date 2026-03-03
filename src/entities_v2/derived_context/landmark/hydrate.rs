use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use super::model::{Landmark, LandmarkType, LandmarkWithParentsAndElements};
use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::element::model::Element;
use crate::entities_v2::shared::MaturingState;
use crate::schema::{landmarks, landscape_landmarks};

type LandmarkTuple = (
    Uuid,
    Uuid,
    Option<Uuid>,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    NaiveDateTime,
    NaiveDateTime,
);

fn tuple_to_landmark(row: LandmarkTuple) -> Landmark {
    let (
        id,
        _analysis_id,
        _parent_id,
        title,
        subtitle,
        content,
        external_content_url,
        comment,
        image_url,
        landmark_type_raw,
        maturing_state_raw,
        created_at,
        updated_at,
    ) = row;

    Landmark {
        id,
        title,
        subtitle,
        content,
        external_content_url,
        comment,
        image_url,
        landmark_type: LandmarkType::from_code(&landmark_type_raw).unwrap_or(LandmarkType::Resource),
        maturing_state: MaturingState::from_code(&maturing_state_raw).unwrap_or(MaturingState::Draft),
        created_at,
        updated_at,
    }
}

fn select_landmark_columns() -> (
    landmarks::id,
    landmarks::analysis_id,
    landmarks::parent_id,
    landmarks::title,
    landmarks::subtitle,
    landmarks::content,
    landmarks::external_content_url,
    landmarks::comment,
    landmarks::image_url,
    landmarks::landmark_type,
    landmarks::maturing_state,
    landmarks::created_at,
    landmarks::updated_at,
) {
    (
        landmarks::id,
        landmarks::analysis_id,
        landmarks::parent_id,
        landmarks::title,
        landmarks::subtitle,
        landmarks::content,
        landmarks::external_content_url,
        landmarks::comment,
        landmarks::image_url,
        landmarks::landmark_type,
        landmarks::maturing_state,
        landmarks::created_at,
        landmarks::updated_at,
    )
}

impl Landmark {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Landmark, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let row = landmarks::table
            .filter(landmarks::id.eq(id))
            .select(select_landmark_columns())
            .first::<LandmarkTuple>(&mut conn)
            .optional()?;
        row.map(tuple_to_landmark).ok_or_else(|| {
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Landmark not found".to_string(),
            )
        })
    }

    pub fn find_parent(&self, pool: &DbPool) -> Result<Option<Landmark>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let parent_id = landmarks::table
            .filter(landmarks::id.eq(self.id))
            .select(landmarks::parent_id)
            .first::<Option<Uuid>>(&mut conn)?;
        if let Some(parent_id) = parent_id {
            return Landmark::find(parent_id, pool).map(Some);
        }
        Ok(None)
    }

    pub fn find_elements(&self, pool: &DbPool) -> Result<Vec<Element>, PpdcError> {
        Element::find_for_landmark(self.id, pool)
    }

    pub fn find_with_parents(
        id: Uuid,
        pool: &DbPool,
    ) -> Result<LandmarkWithParentsAndElements, PpdcError> {
        let origin_landmark = Landmark::find(id, pool)?;
        let mut elements: Vec<Element> = origin_landmark.find_elements(pool)?;
        let mut parents: Vec<Landmark> = vec![];
        let mut current_landmark = origin_landmark.clone();
        while let Some(parent) = current_landmark.find_parent(pool)? {
            let current_elements = parent.find_elements(pool)?;
            elements.extend(current_elements);
            parents.push(parent.clone());
            current_landmark = parent;
        }
        Ok(LandmarkWithParentsAndElements::new(
            origin_landmark,
            parents,
            elements,
        ))
    }

    pub fn get_for_landscape_analysis(
        landscape_analysis_id: Uuid,
        relation_type: Option<&str>,
        pool: &DbPool,
    ) -> Result<Vec<Landmark>, PpdcError> {
        let relation_filter = relation_type.map(|value| match value {
            "ownr" => "OWNED_BY_ANALYSIS",
            "refr" => "REFERENCED",
            other => other,
        });
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let mut query = landscape_landmarks::table
            .filter(landscape_landmarks::landscape_analysis_id.eq(landscape_analysis_id))
            .into_boxed();
        if let Some(filter) = relation_filter {
            query = query.filter(landscape_landmarks::relation_type.eq(filter));
        }

        let landmark_ids = query
            .select(landscape_landmarks::landmark_id)
            .load::<Uuid>(&mut conn)?;

        landmark_ids
            .into_iter()
            .map(|landmark_id| Landmark::find(landmark_id, pool))
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn find_high_level_projects_for_user(
        user_id: Uuid,
        pool: &DbPool,
    ) -> Result<Vec<Landmark>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let landmark_ids = landmarks::table
            .filter(landmarks::user_id.eq(user_id))
            .filter(landmarks::landmark_type.eq("HIGH_LEVEL_PROJECT"))
            .select(landmarks::id)
            .load::<Uuid>(&mut conn)?;
        landmark_ids
            .into_iter()
            .map(|id| Landmark::find(id, pool))
            .collect()
    }
}
