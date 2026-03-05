ALTER TABLE users
ADD COLUMN IF NOT EXISTS timezone TEXT;

UPDATE users
SET timezone = 'UTC'
WHERE timezone IS NULL
   OR btrim(timezone) = '';

ALTER TABLE users
ALTER COLUMN timezone SET DEFAULT 'UTC';

ALTER TABLE users
ALTER COLUMN timezone SET NOT NULL;

ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_timezone_not_blank_check;

ALTER TABLE users
ADD CONSTRAINT users_timezone_not_blank_check
CHECK (btrim(timezone) <> '');
