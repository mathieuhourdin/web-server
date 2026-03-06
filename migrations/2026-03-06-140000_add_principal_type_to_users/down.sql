ALTER TABLE users
ALTER COLUMN principal_type DROP DEFAULT;

ALTER TABLE users
DROP COLUMN principal_type;
