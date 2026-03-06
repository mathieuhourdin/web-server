use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, session::Session, trace_mirror::TraceMirror};

use super::model::Reference;

#[debug_handler]
pub async fn get_trace_mirror_references_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Reference>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let trace_mirror = TraceMirror::find_full_trace_mirror(id, &pool)?;
    if trace_mirror.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let references = Reference::find_for_trace_mirror(id, &pool)?;
    Ok(Json(references))
}
