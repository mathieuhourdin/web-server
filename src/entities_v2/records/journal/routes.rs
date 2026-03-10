use axum::{
    debug_handler,
    extract::{Extension, Json, Multipart, Path},
};
use chrono::Utc;
use serde_json::json;
use std::collections::HashSet;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    message::Message,
    records::journal_import::{model::ImportJournalResult, service::import_journal_text},
    session::Session,
    trace::Trace,
};

use super::model::{
    Journal, JournalExportDto, JournalExportFormat, JournalExportResponse, NewJournalDto,
    UpdateJournalDto,
};

#[debug_handler]
pub async fn get_user_journals_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<Journal>>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if session_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    let journals = Journal::find_for_user(user_id, &pool)?;
    Ok(Json(journals))
}

#[debug_handler]
pub async fn post_journal_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewJournalDto>,
) -> Result<Json<Journal>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::create(payload, user_id, &pool)?;
    Ok(Json(journal))
}

#[debug_handler]
pub async fn put_journal_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateJournalDto>,
) -> Result<Json<Journal>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let mut journal = Journal::find_full(id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    if let Some(title) = payload.title {
        journal.title = title;
    }
    if let Some(subtitle) = payload.subtitle {
        journal.subtitle = subtitle;
    }
    if let Some(content) = payload.content {
        journal.content = content;
    }
    if let Some(is_encrypted) = payload.is_encrypted {
        journal.is_encrypted = is_encrypted;
    }
    if let Some(journal_type) = payload.journal_type {
        journal.journal_type = journal_type;
    }
    if let Some(status) = payload.status {
        journal.status = status;
    }

    let journal = journal.update(&pool)?;
    Ok(Json(journal))
}

#[debug_handler]
pub async fn post_journal_import_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<Json<ImportJournalResult>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;

    let mut preferred_file_bytes: Option<Vec<u8>> = None;
    let mut fallback_file_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Multipart error: {}", err),
        )
    })? {
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

    let file_bytes = preferred_file_bytes
        .or(fallback_file_bytes)
        .ok_or_else(|| {
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

#[debug_handler]
pub async fn post_journal_export_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<JournalExportDto>,
) -> Result<Json<JournalExportResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(id, &pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let mut traces = Trace::get_all_for_journal(journal.id, &pool)?;
    traces.sort_by_key(|trace| (trace.interaction_date, trace.created_at));

    let messages = if payload.include_messages {
        let mut seen = HashSet::new();
        let mut collected = Vec::new();
        for trace in &traces {
            let trace_messages = Message::find_for_trace_conversation(user_id, trace.id, 500, &pool)?;
            for message in trace_messages {
                if seen.insert(message.id) {
                    collected.push(message);
                }
            }
        }
        collected.sort_by_key(|message| message.created_at);
        collected
    } else {
        vec![]
    };

    let content = match payload.format {
        JournalExportFormat::Markdown => render_markdown_export(&journal, &traces, &messages),
        JournalExportFormat::Text => render_text_export(&journal, &traces, &messages),
        JournalExportFormat::Json => render_json_export(&journal, &traces, &messages)?,
    };

    Ok(Json(JournalExportResponse {
        format: payload.format,
        content,
    }))
}

fn render_markdown_export(journal: &Journal, traces: &[Trace], messages: &[Message]) -> String {
    let mut out = String::new();
    out.push_str("# Journal Export\n");
    out.push_str(format!("exported_at: {}\n", Utc::now().naive_utc()).as_str());
    out.push_str(format!("journal_id: {}\n", journal.id).as_str());
    out.push_str(format!("journal_title: {}\n", journal.title).as_str());
    out.push_str(format!("journal_type: {}\n", journal.journal_type.to_db()).as_str());
    out.push('\n');

    for trace in traces {
        out.push_str(format!("export_generated_date: {}\n", trace.interaction_date).as_str());
        out.push_str(trace.content.as_str());
        out.push('\n');
        out.push('\n');
    }

    if !messages.is_empty() {
        out.push_str("# Messages\n\n");
        for message in messages {
            out.push_str(format!("message_date: {}\n", message.created_at).as_str());
            out.push_str(format!("message_type: {}\n", message.message_type.to_db()).as_str());
            if let Some(trace_id) = message.trace_id {
                out.push_str(format!("trace_id: {}\n", trace_id).as_str());
            }
            out.push_str(format!("title: {}\n", message.title).as_str());
            out.push('\n');
            out.push_str(message.content.as_str());
            out.push('\n');
            out.push('\n');
        }
    }

    out
}

fn render_text_export(journal: &Journal, traces: &[Trace], messages: &[Message]) -> String {
    let mut out = String::new();
    out.push_str("Journal Export\n");
    out.push_str(format!("exported_at: {}\n", Utc::now().naive_utc()).as_str());
    out.push_str(format!("journal_id: {}\n", journal.id).as_str());
    out.push_str(format!("journal_title: {}\n", journal.title).as_str());
    out.push_str(format!("journal_type: {}\n", journal.journal_type.to_db()).as_str());
    out.push('\n');

    for trace in traces {
        out.push_str(format!("export_generated_date: {}\n", trace.interaction_date).as_str());
        out.push_str(trace.content.as_str());
        out.push('\n');
        out.push('\n');
    }

    if !messages.is_empty() {
        out.push_str("Messages\n\n");
        for message in messages {
            out.push_str(format!("message_date: {}\n", message.created_at).as_str());
            out.push_str(format!("message_type: {}\n", message.message_type.to_db()).as_str());
            if let Some(trace_id) = message.trace_id {
                out.push_str(format!("trace_id: {}\n", trace_id).as_str());
            }
            out.push_str(format!("title: {}\n", message.title).as_str());
            out.push('\n');
            out.push_str(message.content.as_str());
            out.push('\n');
            out.push('\n');
        }
    }

    out
}

fn render_json_export(
    journal: &Journal,
    traces: &[Trace],
    messages: &[Message],
) -> Result<String, PpdcError> {
    let payload = json!({
        "exported_at": Utc::now().naive_utc(),
        "journal": journal,
        "traces": traces,
        "messages": messages
    });
    Ok(serde_json::to_string_pretty(&payload)?)
}
