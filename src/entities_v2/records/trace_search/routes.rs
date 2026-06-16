use axum::{
    debug_handler,
    extract::{Extension, Json, Query, RawQuery},
};

use crate::db::DbPool;
use crate::entities_v2::{error::PpdcError, session::Session};
use crate::pagination::{parse_repeated_query_param, PaginatedResponse};

use super::model::{TraceSearchDocument, TraceSearchItem, TraceSearchParams};

#[debug_handler]
pub async fn get_trace_search_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    RawQuery(raw_query): RawQuery,
    Query(params): Query<TraceSearchParams>,
) -> Result<Json<PaginatedResponse<TraceSearchItem>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.pagination.validate()?;
    let journal_ids = parse_repeated_query_param(raw_query.as_deref(), "journal_id")?;
    let landmark_ids = parse_repeated_query_param(raw_query.as_deref(), "landmark_id")?;
    let high_level_project_landmark_ids =
        parse_repeated_query_param(raw_query.as_deref(), "high_level_project_landmark_id")?;
    let (items, total) = TraceSearchDocument::search_for_user(
        user_id,
        &params.q,
        &journal_ids,
        &landmark_ids,
        &high_level_project_landmark_ids,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;

    Ok(Json(PaginatedResponse::new(items, pagination, total)))
}
