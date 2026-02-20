use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::entities_v2::landmark::Landmark;

use super::model::{Element, ElementRelationWithRelatedElement};

#[debug_handler]
pub async fn get_element_landmarks_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Landmark>>, PpdcError> {
    let element = Element::find(id, &pool)?;
    let landmarks = element.find_landmarks(&pool)?;
    Ok(Json(landmarks))
}

#[debug_handler]
pub async fn get_element_relations_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ElementRelationWithRelatedElement>>, PpdcError> {
    let element = Element::find(id, &pool)?;
    let relations = element.find_related_elements(&pool)?;
    Ok(Json(relations))
}
