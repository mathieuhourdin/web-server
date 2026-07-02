use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, session::Session};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::hydrate::LandmarkReferenceTypeFilter;
use super::model::LandmarkReferenceListItem;
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

#[debug_handler]
pub async fn get_me_landmarks_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<LandmarkReferenceListItem>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (items, total) = Landmark::find_reference_ranked_for_current_lens(
        user_id,
        LandmarkReferenceTypeFilter::NonHighLevelProject,
        pagination.limit,
        pagination.offset,
        &pool,
    )?;

    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}

#[debug_handler]
pub async fn get_me_high_level_projects_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<LandmarkReferenceListItem>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (items, total) = Landmark::find_reference_ranked_for_current_lens(
        user_id,
        LandmarkReferenceTypeFilter::HighLevelProject,
        pagination.limit,
        pagination.offset,
        &pool,
    )?;

    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}
