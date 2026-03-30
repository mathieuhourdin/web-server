DROP INDEX IF EXISTS idx_messages_post_id_created_at;

ALTER TABLE messages
DROP COLUMN IF EXISTS post_id;
