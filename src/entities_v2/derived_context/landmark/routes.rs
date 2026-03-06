use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, session::Session};

use super::model::{Landmark, LandmarkWithParentsAndElements};

#[debug_handler]
pub async fn get_landmark_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<LandmarkWithParentsAndElements>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let landmark_user_id = Landmark::find_user_id(id, &pool)?;
    if landmark_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let landmark = Landmark::find_with_parents(id, &pool)?;
    Ok(Json(landmark))
}
