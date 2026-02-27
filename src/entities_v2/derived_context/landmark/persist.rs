use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;

use super::model::{Landmark, NewLandmark};

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
    let landmark = landmark.create(pool)?;
    Ok(landmark)
}
