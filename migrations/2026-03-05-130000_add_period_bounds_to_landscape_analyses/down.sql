ALTER TABLE landscape_analyses
ALTER COLUMN period_start DROP DEFAULT;

ALTER TABLE landscape_analyses
ALTER COLUMN period_end DROP DEFAULT;

ALTER TABLE landscape_analyses
DROP COLUMN IF EXISTS period_start;

ALTER TABLE landscape_analyses
DROP COLUMN IF EXISTS period_end;
