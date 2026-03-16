DROP INDEX IF EXISTS idx_messages_recipient_seen_at;

ALTER TABLE messages
DROP COLUMN IF EXISTS seen_at;
