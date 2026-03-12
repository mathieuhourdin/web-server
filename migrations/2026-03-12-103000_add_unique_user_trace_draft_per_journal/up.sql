UPDATE traces
SET status = 'FINALIZED',
    finalized_at = COALESCE(finalized_at, updated_at)
WHERE status = 'DRAFT'
  AND trace_type = 'USER_TRACE';

CREATE UNIQUE INDEX traces_one_draft_user_trace_per_journal_idx
ON traces (journal_id)
WHERE status = 'DRAFT' AND trace_type = 'USER_TRACE';
