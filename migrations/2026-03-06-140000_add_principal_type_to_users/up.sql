ALTER TABLE users
ADD COLUMN principal_type TEXT;

UPDATE users
SET principal_type = 'HUMAN'
WHERE principal_type IS NULL;

ALTER TABLE users
ALTER COLUMN principal_type SET NOT NULL;

ALTER TABLE users
ALTER COLUMN principal_type SET DEFAULT 'HUMAN';
