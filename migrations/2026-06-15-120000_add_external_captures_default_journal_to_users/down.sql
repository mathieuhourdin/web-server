DROP INDEX IF EXISTS idx_users_external_captures_default_journal_id;

ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_external_captures_default_journal_id_fkey;

ALTER TABLE users
DROP COLUMN IF EXISTS external_captures_default_journal_id;
