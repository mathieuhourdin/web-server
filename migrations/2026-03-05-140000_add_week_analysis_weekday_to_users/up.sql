ALTER TABLE users
ADD COLUMN IF NOT EXISTS week_analysis_weekday TEXT;

UPDATE users
SET week_analysis_weekday = 'SUNDAY'
WHERE week_analysis_weekday IS NULL;

ALTER TABLE users
ALTER COLUMN week_analysis_weekday SET DEFAULT 'SUNDAY';

ALTER TABLE users
ALTER COLUMN week_analysis_weekday SET NOT NULL;

ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_week_analysis_weekday_check;

ALTER TABLE users
ADD CONSTRAINT users_week_analysis_weekday_check
CHECK (
    week_analysis_weekday IN (
        'MONDAY',
        'TUESDAY',
        'WEDNESDAY',
        'THURSDAY',
        'FRIDAY',
        'SATURDAY',
        'SUNDAY'
    )
);
