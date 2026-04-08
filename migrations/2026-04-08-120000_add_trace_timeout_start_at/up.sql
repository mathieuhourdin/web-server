ALTER TABLE traces
ADD COLUMN timeout_start_at TIMESTAMPTZ NULL;

UPDATE traces
SET timeout_start_at = start_writing_at AT TIME ZONE 'UTC'
WHERE timeout_at IS NOT NULL
  AND timeout_start_at IS NULL;
