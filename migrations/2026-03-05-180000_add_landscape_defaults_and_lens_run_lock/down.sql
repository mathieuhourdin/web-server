DROP INDEX IF EXISTS idx_lenses_run_lock_until;

ALTER TABLE lenses
DROP COLUMN IF EXISTS run_lock_until;

ALTER TABLE lenses
DROP COLUMN IF EXISTS run_lock_owner;

ALTER TABLE landscape_analyses
ALTER COLUMN landscape_analysis_type DROP DEFAULT;

ALTER TABLE landscape_analyses
ALTER COLUMN processing_state DROP DEFAULT;
