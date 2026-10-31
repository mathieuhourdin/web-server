use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::journal::Journal;

use super::{
    model::{ImportBlock, ImportJournalResult, ImportTraceFailure},
    parser::{blocks_to_new_traces, extract_blocks, imported_date, is_date_heading},
};

fn purpose_block_to_subtitle(block: &ImportBlock) -> String {
    let mut lines = block.content.lines();
    let first_line = lines.next().unwrap_or_default();
    if first_line.trim() == block.header.trim() {
        let remaining = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        if !remaining.is_empty() {
            return remaining;
        }
    }
    block.content.trim().to_string()
}

pub fn import_journal_text(
    user_id: Uuid,
    journal_id: Uuid,
    raw_text: &str,
    pool: &DbPool,
) -> Result<ImportJournalResult, PpdcError> {
    let journal = Journal::find_full(journal_id, pool)?;
    if journal.user_id != user_id {
        return Err(PpdcError::unauthorized());
    }

    let mut blocks = extract_blocks(raw_text)
        .into_iter()
        .filter(|block| !block.header.trim().is_empty())
        .collect::<Vec<_>>();
    if blocks.is_empty() {
        return Err(PpdcError::new(
            400,
            ErrorType::ApiError,
            "No importable blocks found".to_string(),
        ));
    }

    if blocks
        .first()
        .is_some_and(|block| block.date.is_none() && !is_date_heading(&block.header))
    {
        let purpose_block = blocks.remove(0);
        let subtitle_candidate = purpose_block_to_subtitle(&purpose_block);
        if !subtitle_candidate.is_empty() {
            let mut journal = Journal::find_full(journal_id, pool)?;
            if journal.subtitle.trim().is_empty() {
                journal.subtitle = subtitle_candidate;
                journal.update(pool)?;
            }
        }
    }

    let user = crate::entities_v2::user::User::find(&user_id, pool)?;
    let timezone = user
        .timezone
        .parse::<chrono_tz::Tz>()
        .unwrap_or(chrono_tz::UTC);
    let traces = blocks_to_new_traces(blocks, user_id, journal_id, timezone);
    let total_blocks = traces.len();
    let mut created_trace_ids = Vec::new();
    let mut failures = Vec::new();
    let mut date_fallback_block_indices = Vec::new();

    for (block_index, block, trace) in traces {
        if imported_date(&block, timezone).is_none() {
            date_fallback_block_indices.push(block_index);
        }
        let imported_date = trace
            .start_writing_at
            .unwrap_or_else(|| chrono::Utc::now().naive_utc());
        match trace.create_imported(imported_date, pool) {
            Ok(finalized_trace) => created_trace_ids.push(finalized_trace.id),
            Err(err) => failures.push(ImportTraceFailure {
                block_index,
                header: block.header,
                error: err.message,
            }),
        }
    }

    Ok(ImportJournalResult {
        created_count: created_trace_ids.len(),
        failed_count: failures.len(),
        total_blocks,
        created_trace_ids,
        failures,
        date_fallback_block_indices,
    })
}
