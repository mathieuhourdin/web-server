use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::{
    error::{ErrorType, PpdcError},
    interaction::model::Interaction,
};
use crate::entities_v2::journal::Journal;

use super::{
    model::{ImportBlock, ImportJournalResult, ImportTraceFailure},
    parser::{blocks_to_new_traces, extract_blocks},
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
    Interaction::find_user_journal(user_id, journal_id, pool)?;

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

    if blocks.first().is_some_and(|block| block.date.is_none()) {
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

    let traces = blocks_to_new_traces(blocks, user_id, journal_id);
    let total_blocks = traces.len();
    let mut created_trace_ids = Vec::new();
    let mut failures = Vec::new();

    for (block_index, block, trace) in traces {
        match trace.create(pool) {
            Ok(created_trace) => created_trace_ids.push(created_trace.id),
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
    })
}
