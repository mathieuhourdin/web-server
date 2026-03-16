ALTER TABLE messages
ADD COLUMN seen_at TIMESTAMP NULL;

CREATE INDEX IF NOT EXISTS idx_messages_recipient_seen_at
    ON messages(recipient_user_id, seen_at, created_at DESC);
