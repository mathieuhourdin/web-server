use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::PpdcError;

use super::model::Journal;

#[debug_handler]
pub async fn get_user_journals_route(
    Extension(pool): Extension<DbPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<Journal>>, PpdcError> {
    let journals = Journal::find_for_user(user_id, &pool)?;
    Ok(Json(journals))
}
