use axum::{
    debug_handler,
    extract::{Extension, Json, Multipart, Path, Query},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
};
use chrono::{Datelike, NaiveDate, Utc};
use serde_json::json;
use std::collections::HashSet;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    journal_sharing_policy::JournalSharingPolicy,
    message::Message,
    records::journal_import::{model::ImportJournalResult, service::import_journal_text},
    session::Session,
    trace::{Trace, TraceStatus},
    user::{ensure_user_has_default_journals, ensure_user_has_meta_journal},
};
use crate::pagination::{PaginatedResponse, PaginationParams};

use super::model::{
    Journal, JournalExportDto, JournalExportFormat, JournalExportResponse, JournalType,
    NewJournalDto, UpdateJournalDto,
};

#[derive(serde::Deserialize)]
pub struct UserJournalsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub journal_type: Option<JournalType>,
}

#[derive(serde::Deserialize)]
pub struct AllTracesExportQuery {
    pub format: JournalExportFormat,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

struct DatedTraceExportItem {
    trace: Trace,
    journal_title: String,
}

#[debug_handler]
pub async fn get_user_journals_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<UserJournalsQuery>,
) -> Result<Json<PaginatedResponse<Journal>>, PpdcError> {
    let session_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if session_user_id != user_id {
        return Err(PpdcError::unauthorized());
    }
    ensure_user_has_default_journals(user_id, &pool)?;
    ensure_user_has_meta_journal(user_id, &pool)?;
    let pagination = params.pagination.validate()?;
    let journal_type = params.journal_type.unwrap_or(JournalType::UserJournal);
    let (journals, total) = Journal::find_for_user_paginated(
        user_id,
        pagination.offset,
        pagination.limit,
        journal_type,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(journals, pagination, total)))
}

#[debug_handler]
pub async fn get_shared_journals_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Journal>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let shared_ids = JournalSharingPolicy::find_shared_journal_ids_for_user(user_id, &pool)?;
    let (journals, total) =
        Journal::find_many_paginated(shared_ids, pagination.offset, pagination.limit, true, &pool)?;
    Ok(Json(PaginatedResponse::new(journals, pagination, total)))
}

#[debug_handler]
pub async fn get_recent_shared_journals_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Journal>>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (journals, total) = Journal::find_recent_shared_for_user_paginated(
        user_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(journals, pagination, total)))
}

#[debug_handler]
pub async fn get_user_recent_shared_journals_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_owner_user_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Journal>>, PpdcError> {
    let viewer_user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let pagination = params.validate()?;
    let (journals, total) = Journal::find_recent_shared_for_owner_paginated(
        viewer_user_id,
        journal_owner_user_id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;
    Ok(Json(PaginatedResponse::new(journals, pagination, total)))
}

#[debug_handler]
pub async fn get_journal_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Journal>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(id, &pool)?;
    if !JournalSharingPolicy::user_can_read_journal(&journal, user_id, &pool)? {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(journal))
}

#[debug_handler]
pub async fn post_journal_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(payload): Json<NewJournalDto>,
) -> Result<Json<Journal>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::create(payload, user_id, &pool)?;
    JournalSharingPolicy::create_missing_policies_for_existing_followers(&journal, &pool)?;
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
        if is_encrypted && JournalSharingPolicy::has_active_policies_for_journal(journal.id, &pool)?
        {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Revoke active journal sharing policies before enabling encryption".to_string(),
            ));
        }
        journal.is_encrypted = is_encrypted;
    }
    if let Some(journal_type) = payload.journal_type {
        journal.journal_type = journal_type;
    }
    if let Some(sharing_mode) = payload.sharing_mode {
        journal.sharing_mode = sharing_mode;
    }
    if let Some(status) = payload.status {
        journal.status = status;
    }

    let journal = journal.update(&pool)?;
    JournalSharingPolicy::create_missing_policies_for_existing_followers(&journal, &pool)?;
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
            let trace_messages =
                Message::find_for_trace_conversation(user_id, trace.id, 500, &pool)?;
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

#[debug_handler]
pub async fn get_all_my_traces_export_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(params): Query<AllTracesExportQuery>,
) -> Result<Response, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if params.from.is_some_and(|from| params.to.is_some_and(|to| from > to)) {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "from must be earlier than or equal to to".to_string(),
        ));
    }
    if params.format == JournalExportFormat::Json {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "format must be md or txt".to_string(),
        ));
    }

    let journals = Journal::find_all_owned_by(user_id, &pool)?;
    let mut items = Vec::new();
    for journal in journals {
        for status in [
            TraceStatus::Draft,
            TraceStatus::Finalized,
            TraceStatus::Archived,
        ] {
            let (traces, _) = Trace::get_for_journal_paginated(
                journal.id,
                user_id,
                0,
                i64::MAX / 4,
                None,
                status,
                None,
                &pool,
            )?;
            for trace in traces {
                if trace.status == TraceStatus::Draft && trace.is_blank {
                    continue;
                }
                let trace_date = trace.interaction_date.date();
                if params.from.is_some_and(|from| trace_date < from)
                    || params.to.is_some_and(|to| trace_date > to)
                {
                    continue;
                }
                items.push(DatedTraceExportItem {
                    trace,
                    journal_title: journal.title.clone(),
                });
            }
        }
    }
    items.sort_by(|left, right| {
        left.trace
            .interaction_date
            .cmp(&right.trace.interaction_date)
            .then_with(|| left.trace.created_at.cmp(&right.trace.created_at))
            .then_with(|| left.journal_title.cmp(&right.journal_title))
            .then_with(|| left.trace.id.cmp(&right.trace.id))
    });

    let exported_at = Utc::now().naive_utc();
    let (content_type, extension, content) = match params.format {
        JournalExportFormat::Markdown => (
            "text/markdown; charset=utf-8",
            "md",
            render_all_traces_markdown(&items, exported_at, params.from, params.to),
        ),
        JournalExportFormat::Text => (
            "text/plain; charset=utf-8",
            "txt",
            render_all_traces_text(&items, exported_at, params.from, params.to),
        ),
        JournalExportFormat::Json => unreachable!(),
    };
    let filename = format!(
        "hupo-traces-{}.{}",
        exported_at.date().format("%Y-%m-%d"),
        extension
    );
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename)).map_err(
            |err| {
                PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!("Failed to build export filename header: {}", err),
                )
            },
        )?,
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok((headers, content).into_response())
}

fn single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn export_filter_description(from: Option<NaiveDate>, to: Option<NaiveDate>) -> String {
    match (from, to) {
        (Some(from), Some(to)) => format!("{} through {} (inclusive)", from, to),
        (Some(from), None) => format!("from {} (inclusive)", from),
        (None, Some(to)) => format!("through {} (inclusive)", to),
        (None, None) => "all dates".to_string(),
    }
}

fn render_all_traces_markdown(
    items: &[DatedTraceExportItem],
    exported_at: chrono::NaiveDateTime,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> String {
    let mut out = format!(
        "# My Hupo traces\n\nexported_at: {}\ndate_filter: {}\ntrace_count: {}\n\n",
        exported_at,
        export_filter_description(from, to),
        items.len()
    );
    let mut current_month = None;
    for item in items {
        let date = item.trace.interaction_date;
        let month = (date.year(), date.month());
        if current_month != Some(month) {
            current_month = Some(month);
            out.push_str(&format!("## {:04}-{:02}\n\n", month.0, month.1));
        }
        let title = single_line(&item.trace.title);
        let title = if title.is_empty() {
            "Untitled trace".to_string()
        } else {
            title
        };
        out.push_str(&format!(
            "### {} — {}\n\n_Journal: {}_\n\n",
            date.format("%Y-%m-%d %H:%M"),
            title,
            single_line(&item.journal_title)
        ));
        if item.trace.is_encrypted {
            out.push_str("_Encrypted content as stored_\n\n");
        }
        out.push_str(&item.trace.content);
        out.push_str("\n\n");
    }
    out
}

fn render_all_traces_text(
    items: &[DatedTraceExportItem],
    exported_at: chrono::NaiveDateTime,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> String {
    let mut out = format!(
        "MY HUPO TRACES\nExported at: {}\nDate filter: {}\nTrace count: {}\n\n",
        exported_at,
        export_filter_description(from, to),
        items.len()
    );
    for item in items {
        let title = single_line(&item.trace.title);
        let title = if title.is_empty() {
            "Untitled trace".to_string()
        } else {
            title
        };
        out.push_str("==================================================\n");
        out.push_str(&format!(
            "{} — {}\nJournal: {}\n",
            item.trace.interaction_date.format("%Y-%m-%d %H:%M"),
            title,
            single_line(&item.journal_title)
        ));
        if item.trace.is_encrypted {
            out.push_str("Encrypted content as stored\n");
        }
        out.push('\n');
        out.push_str(&item.trace.content);
        out.push_str("\n\n");
    }
    out
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
        out.push_str(trace.title.as_str());
        out.push('\n');
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
