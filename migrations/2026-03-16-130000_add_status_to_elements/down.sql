DROP INDEX IF EXISTS idx_elements_status_created_at;

ALTER TABLE elements
DROP COLUMN IF EXISTS status;
