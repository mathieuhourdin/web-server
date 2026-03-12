ALTER TABLE traces
ADD COLUMN start_writing_at TIMESTAMP;

UPDATE traces
SET start_writing_at = created_at;

ALTER TABLE traces
ALTER COLUMN start_writing_at SET NOT NULL;

ALTER TABLE traces
ADD COLUMN finalized_at TIMESTAMP NULL;

UPDATE traces
SET finalized_at = updated_at
WHERE status IN ('FINALIZED', 'ARCHIVED');
