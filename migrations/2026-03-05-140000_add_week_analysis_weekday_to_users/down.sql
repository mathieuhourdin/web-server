ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_week_analysis_weekday_check;

ALTER TABLE users
ALTER COLUMN week_analysis_weekday DROP DEFAULT;

ALTER TABLE users
DROP COLUMN IF EXISTS week_analysis_weekday;
