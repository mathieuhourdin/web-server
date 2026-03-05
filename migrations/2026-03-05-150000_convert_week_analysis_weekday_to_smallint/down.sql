ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_week_analysis_weekday_check;

ALTER TABLE users
ALTER COLUMN week_analysis_weekday DROP DEFAULT;

ALTER TABLE users
ALTER COLUMN week_analysis_weekday TYPE TEXT
USING (
    CASE week_analysis_weekday
        WHEN 1 THEN 'MONDAY'
        WHEN 2 THEN 'TUESDAY'
        WHEN 3 THEN 'WEDNESDAY'
        WHEN 4 THEN 'THURSDAY'
        WHEN 5 THEN 'FRIDAY'
        WHEN 6 THEN 'SATURDAY'
        WHEN 7 THEN 'SUNDAY'
        ELSE 'SUNDAY'
    END
);

ALTER TABLE users
ALTER COLUMN week_analysis_weekday SET DEFAULT 'SUNDAY';

ALTER TABLE users
ALTER COLUMN week_analysis_weekday SET NOT NULL;

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
