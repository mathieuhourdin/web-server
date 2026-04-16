ALTER TABLE users
ADD COLUMN IF NOT EXISTS shared_journal_activity_email_mode TEXT;

UPDATE users
SET shared_journal_activity_email_mode = 'instant'
WHERE shared_journal_activity_email_mode IS NULL
   OR btrim(shared_journal_activity_email_mode) = '';

ALTER TABLE users
ALTER COLUMN shared_journal_activity_email_mode SET DEFAULT 'instant';

ALTER TABLE users
ALTER COLUMN shared_journal_activity_email_mode SET NOT NULL;

ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_shared_journal_activity_email_mode_check;

ALTER TABLE users
ADD CONSTRAINT users_shared_journal_activity_email_mode_check
CHECK (shared_journal_activity_email_mode IN ('off', 'instant', 'daily_digest'));

ALTER TABLE users
ADD COLUMN IF NOT EXISTS received_message_email_mode TEXT;

UPDATE users
SET received_message_email_mode = 'instant'
WHERE received_message_email_mode IS NULL
   OR btrim(received_message_email_mode) = '';

ALTER TABLE users
ALTER COLUMN received_message_email_mode SET DEFAULT 'instant';

ALTER TABLE users
ALTER COLUMN received_message_email_mode SET NOT NULL;

ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_received_message_email_mode_check;

ALTER TABLE users
ADD CONSTRAINT users_received_message_email_mode_check
CHECK (received_message_email_mode IN ('off', 'instant', 'daily_digest'));
