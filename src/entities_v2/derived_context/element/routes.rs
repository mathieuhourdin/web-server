use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::landmark::Landmark;
use crate::entities_v2::{error::PpdcError, session::Session};

use super::model::{Element, ElementRelationWithRelatedElement};

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
