DROP INDEX IF EXISTS idx_users_mentor_id;

ALTER TABLE users
DROP COLUMN mentor_id;
