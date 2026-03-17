use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query, RawQuery},
};
use chrono::NaiveDateTime;
use serde::Deserialize;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::landmark::Landmark;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    lens::Lens,
    session::Session,
    user::User,
};

use super::model::{Element, ElementRelationWithRelatedElement, ElementStatus};

#[derive(Deserialize)]
pub struct ElementFiltersQuery {
    pub interaction_date_from: Option<NaiveDateTime>,
    pub interaction_date_to: Option<NaiveDateTime>,
    pub created_at_from: Option<NaiveDateTime>,
    pub created_at_to: Option<NaiveDateTime>,
}

#[debug_handler]
pub async fn get_elements_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    RawQuery(raw_query): RawQuery,
    Query(filters): Query<ElementFiltersQuery>,
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

    let element_type = crate::pagination::parse_repeated_query_param(
        raw_query.as_deref(),
        "element_type",
    )?;
    let element_subtype = crate::pagination::parse_repeated_query_param(
        raw_query.as_deref(),
        "element_subtype",
    )?;
    let status =
        crate::pagination::parse_repeated_query_param::<ElementStatus>(raw_query.as_deref(), "status")?;

    let analysis_ids = lens.get_analysis_scope_ids(&pool)?;
    let elements = Element::find_for_analysis_scope_filtered(
        user_id,
        analysis_ids,
        element_type,
        element_subtype,
        status,
        filters.interaction_date_from,
        filters.interaction_date_to,
        filters.created_at_from,
        filters.created_at_to,
        &pool,
    )?;
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
