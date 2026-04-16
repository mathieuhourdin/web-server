ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_received_message_email_mode_check;

ALTER TABLE users
DROP COLUMN IF EXISTS received_message_email_mode;

ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_shared_journal_activity_email_mode_check;

ALTER TABLE users
DROP COLUMN IF EXISTS shared_journal_activity_email_mode;
