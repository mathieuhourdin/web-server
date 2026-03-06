UPDATE traces
SET interaction_date = created_at
WHERE interaction_date IS NULL;

ALTER TABLE traces
ALTER COLUMN interaction_date SET NOT NULL;
