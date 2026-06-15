ALTER TABLE users
ADD COLUMN IF NOT EXISTS external_captures_default_journal_id UUID NULL;

ALTER TABLE users
ADD CONSTRAINT users_external_captures_default_journal_id_fkey
FOREIGN KEY (external_captures_default_journal_id)
REFERENCES journals(id)
ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_users_external_captures_default_journal_id
ON users (external_captures_default_journal_id);
