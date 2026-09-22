DROP INDEX IF EXISTS wal_days_compilation_started_idx;
DROP INDEX IF EXISTS wal_days_compilation_due_idx;

ALTER TABLE wal_days
    DROP CONSTRAINT IF EXISTS wal_days_compilation_claim_check,
    DROP COLUMN IF EXISTS compilation_last_error,
    DROP COLUMN IF EXISTS compilation_processing_revision,
    DROP COLUMN IF EXISTS compilation_started_at,
    DROP COLUMN IF EXISTS compilation_due_at;
