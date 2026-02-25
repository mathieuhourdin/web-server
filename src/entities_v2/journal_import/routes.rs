use axum::{
    extract::{Extension, Multipart, Path},
    Json,
};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    session::Session,
};

use super::{model::ImportJournalResult, service::import_journal_text};

pub async fn post_import_text_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<Json<ImportJournalResult>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;

    let mut preferred_file_bytes: Option<Vec<u8>> = None;
    let mut fallback_file_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| PpdcError::new(400, ErrorType::ApiError, format!("Multipart error: {}", err)))?
    {
        let field_name = field.name().map(|name| name.to_string());
        let is_file_field = field.file_name().is_some();
        if !is_file_field {
            continue;
        }

        let bytes = field.bytes().await.map_err(|err| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Failed to read multipart field: {}", err),
            )
        })?;

        if field_name.as_deref() == Some("file") {
            preferred_file_bytes = Some(bytes.to_vec());
            break;
        }
        if fallback_file_bytes.is_none() {
            fallback_file_bytes = Some(bytes.to_vec());
        }
    }

    let file_bytes = preferred_file_bytes.or(fallback_file_bytes).ok_or_else(|| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            "No file provided in multipart payload".to_string(),
        )
    })?;

    let raw_text = String::from_utf8_lossy(&file_bytes).to_string();
    let result = import_journal_text(user_id, journal_id, &raw_text, &pool)?;
    Ok(Json(result))
}
