DROP INDEX IF EXISTS users_apple_sub_unique;
DROP INDEX IF EXISTS users_google_sub_unique;

ALTER TABLE users
DROP COLUMN apple_sub,
DROP COLUMN google_sub;
