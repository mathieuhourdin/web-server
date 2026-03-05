ALTER TABLE landscape_analyses
DROP CONSTRAINT IF EXISTS landscape_analyses_landscape_analysis_type_check;

ALTER TABLE landscape_analyses
ALTER COLUMN landscape_analysis_type DROP DEFAULT;

ALTER TABLE landscape_analyses
DROP COLUMN IF EXISTS landscape_analysis_type;
