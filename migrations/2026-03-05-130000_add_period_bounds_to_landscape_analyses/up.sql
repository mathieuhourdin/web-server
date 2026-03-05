ALTER TABLE landscape_analyses
ADD COLUMN IF NOT EXISTS period_start TIMESTAMP;

ALTER TABLE landscape_analyses
ADD COLUMN IF NOT EXISTS period_end TIMESTAMP;

UPDATE landscape_analyses
SET period_start = COALESCE(interaction_date, created_at)
WHERE period_start IS NULL;

UPDATE landscape_analyses
SET period_end = COALESCE(interaction_date, created_at)
WHERE period_end IS NULL;

ALTER TABLE landscape_analyses
ALTER COLUMN period_start SET DEFAULT NOW();

ALTER TABLE landscape_analyses
ALTER COLUMN period_end SET DEFAULT NOW();

ALTER TABLE landscape_analyses
ALTER COLUMN period_start SET NOT NULL;

ALTER TABLE landscape_analyses
ALTER COLUMN period_end SET NOT NULL;
