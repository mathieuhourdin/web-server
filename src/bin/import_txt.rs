//! Local import utility: use exactly the platform's date parser and import policy.
use std::{fs, path::Path};
use uuid::Uuid;
use web_server::{db, entities_v2::records::journal_import::service::import_journal_text};

const USER_ID: Uuid = uuid::uuid!("a647a452-3a80-4a94-97d0-8a36c1c752c1");
const JOURNAL_ID: Uuid = uuid::uuid!("95bacf39-5ca7-4a9f-99c1-eb6d440dade1");

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pool = db::create_pool();
    let file_content = fs::read_to_string(Path::new("./imports/log_lecture.txt"))?;
    let result = import_journal_text(USER_ID, JOURNAL_ID, &file_content, &pool)
        .map_err(|error| anyhow::anyhow!(error.message))?;
    println!(
        "Imported={} failed={} date_fallback_blocks={:?}",
        result.created_count, result.failed_count, result.date_fallback_block_indices
    );
    // Do not print source writings or token-bearing email content.
    if result.failed_count > 0 {
        return Err(anyhow::anyhow!("Some traces failed to import").into());
    }
    Ok(())
}
