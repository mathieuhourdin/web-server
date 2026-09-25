DROP INDEX IF EXISTS lenses_due_automatic_retry_idx;

ALTER TABLE lenses
DROP COLUMN automatic_retry_count,
DROP COLUMN automatic_retry_at,
DROP COLUMN failure_reason;
