use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;

use super::model::Reference;

#[debug_handler]
pub async fn get_trace_mirror_references_route(
    Extension(pool): Extension<DbPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Reference>>, PpdcError> {
    let references = Reference::find_for_trace_mirror(id, &pool)?;
    Ok(Json(references))
}
