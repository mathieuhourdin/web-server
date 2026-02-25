use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ImportBlock {
    pub header: String,
    pub content: String,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportTraceFailure {
    pub block_index: usize,
    pub header: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportJournalResult {
    pub created_count: usize,
    pub failed_count: usize,
    pub total_blocks: usize,
    pub created_trace_ids: Vec<Uuid>,
    pub failures: Vec<ImportTraceFailure>,
}
