use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::landmark::Landmark;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    lens::Lens,
    session::Session,
    user::User,
};

use super::model::{Element, ElementRelationWithRelatedElement};

#[debug_handler]
pub async fn get_elements_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<Element>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let user = User::find(&user_id, &pool)?;
    let current_lens_id = user.current_lens_id.ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "current_lens_id is not set for user".to_string(),
        )
    })?;

    let lens = Lens::find_full_lens(current_lens_id, &pool)?;
    if lens.user_id != Some(user_id) {
        return Err(PpdcError::unauthorized());
    }

    let analysis_ids = lens.get_analysis_scope_ids(&pool)?;
    let elements = Element::find_for_analysis_scope(user_id, analysis_ids, &pool)?;
    Ok(Json(elements))
}

#[debug_handler]
pub async fn get_element_landmarks_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Landmark>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let element = Element::find(id, &pool)?;
    if element.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let landmarks = element.find_landmarks(&pool)?;
    Ok(Json(landmarks))
}

#[debug_handler]
pub async fn get_element_relations_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ElementRelationWithRelatedElement>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let element = Element::find(id, &pool)?;
    if element.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let relations = element.find_related_elements(&pool)?;
    Ok(Json(relations))
}
