ALTER TABLE landscape_analyses
ADD COLUMN IF NOT EXISTS landscape_analysis_type TEXT;

UPDATE landscape_analyses
SET landscape_analysis_type = 'TRACE_INCREMENTAL'
WHERE landscape_analysis_type IS NULL;

ALTER TABLE landscape_analyses
ALTER COLUMN landscape_analysis_type SET DEFAULT 'TRACE_INCREMENTAL';

ALTER TABLE landscape_analyses
ALTER COLUMN landscape_analysis_type SET NOT NULL;

ALTER TABLE landscape_analyses
DROP CONSTRAINT IF EXISTS landscape_analyses_landscape_analysis_type_check;

ALTER TABLE landscape_analyses
ADD CONSTRAINT landscape_analyses_landscape_analysis_type_check
CHECK (landscape_analysis_type IN ('DAILY_RECAP', 'TRACE_INCREMENTAL'));
