ALTER TABLE lenses
ADD COLUMN failure_reason TEXT NULL,
ADD COLUMN automatic_retry_at TIMESTAMP NULL,
ADD COLUMN automatic_retry_count INTEGER NOT NULL DEFAULT 0;

CREATE INDEX lenses_due_automatic_retry_idx
ON lenses (automatic_retry_at)
WHERE processing_state = 'FAILED' AND automatic_retry_count = 0;
