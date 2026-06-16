use axum::{
    debug_handler,
    extract::{Extension, Json, Query},
};

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, session::Session};
use crate::pagination::PaginatedResponse;

use super::model::{TraceSearchDocument, TraceSearchItem, TraceSearchParams};

#[debug_handler]
pub async fn get_trace_search_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<TraceSearchParams>,
) -> Result<Json<PaginatedResponse<TraceSearchItem>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.pagination.validate()?;
    let (items, total) = TraceSearchDocument::search_for_user(
        user_id,
        &params.q,
        params.journal_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;

    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}
