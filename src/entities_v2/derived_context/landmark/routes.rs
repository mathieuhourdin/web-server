use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;

use super::model::{Landmark, LandmarkWithParentsAndElements};

#[debug_handler]
pub async fn get_landmark_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<LandmarkWithParentsAndElements>, PpdcError> {
    let landmark = Landmark::find_with_parents(id, &pool)?;
    Ok(Json(landmark))
}
