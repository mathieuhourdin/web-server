-- Step 1.1: operational defaults for landscape analyses
ALTER TABLE landscape_analyses
ALTER COLUMN processing_state SET DEFAULT 'PENDING';

ALTER TABLE landscape_analyses
ALTER COLUMN landscape_analysis_type SET DEFAULT 'TRACE_INCREMENTAL';

-- Step 1.3: lens run-lock fields
ALTER TABLE lenses
ADD COLUMN IF NOT EXISTS run_lock_owner UUID NULL;

ALTER TABLE lenses
ADD COLUMN IF NOT EXISTS run_lock_until TIMESTAMP NULL;

CREATE INDEX IF NOT EXISTS idx_lenses_run_lock_until
ON lenses (run_lock_until);
