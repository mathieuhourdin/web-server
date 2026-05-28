ALTER TABLE trace_versions
DROP CONSTRAINT IF EXISTS trace_versions_sharing_sensitivity_check,
DROP COLUMN IF EXISTS sharing_sensitivity;

ALTER TABLE traces
DROP CONSTRAINT IF EXISTS traces_sharing_sensitivity_check,
DROP COLUMN IF EXISTS sharing_sensitivity;
