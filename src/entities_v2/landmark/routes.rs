use axum::{debug_handler, extract::{Extension, Json, Path}};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;

use super::model::{Landmark, LandmarkWithParentsAndElements};

#[debug_handler]
pub async fn get_landmarks_for_user_route(
    Extension(pool): Extension<DbPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<Landmark>>, PpdcError> {
    let landmarks = Landmark::find_all_up_to_date_for_user(user_id, &pool)?;
    Ok(Json(landmarks))
}

#[debug_handler]
pub async fn get_landmark_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<LandmarkWithParentsAndElements>, PpdcError> {
    let landmark = Landmark::find_with_parents(id, &pool)?;
    Ok(Json(landmark))
}
