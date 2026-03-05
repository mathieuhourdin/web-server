ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_week_analysis_weekday_check;

ALTER TABLE users
ALTER COLUMN week_analysis_weekday DROP DEFAULT;

ALTER TABLE users
ALTER COLUMN week_analysis_weekday TYPE SMALLINT
USING (
    CASE
        WHEN week_analysis_weekday IN ('MONDAY', 'Monday', 'monday') THEN 1
        WHEN week_analysis_weekday IN ('TUESDAY', 'Tuesday', 'tuesday') THEN 2
        WHEN week_analysis_weekday IN ('WEDNESDAY', 'Wednesday', 'wednesday') THEN 3
        WHEN week_analysis_weekday IN ('THURSDAY', 'Thursday', 'thursday') THEN 4
        WHEN week_analysis_weekday IN ('FRIDAY', 'Friday', 'friday') THEN 5
        WHEN week_analysis_weekday IN ('SATURDAY', 'Saturday', 'saturday') THEN 6
        WHEN week_analysis_weekday IN ('SUNDAY', 'Sunday', 'sunday') THEN 7
        WHEN week_analysis_weekday ~ '^[0-9]+$' THEN week_analysis_weekday::SMALLINT
        ELSE 7
    END
);

UPDATE users
SET week_analysis_weekday = 7
WHERE week_analysis_weekday IS NULL
   OR week_analysis_weekday < 1
   OR week_analysis_weekday > 7;

ALTER TABLE users
ALTER COLUMN week_analysis_weekday SET DEFAULT 7;

ALTER TABLE users
ALTER COLUMN week_analysis_weekday SET NOT NULL;

ALTER TABLE users
ADD CONSTRAINT users_week_analysis_weekday_check
CHECK (week_analysis_weekday BETWEEN 1 AND 7);
