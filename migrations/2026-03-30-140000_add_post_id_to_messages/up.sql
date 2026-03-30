ALTER TABLE messages
ADD COLUMN post_id UUID REFERENCES posts(id);

CREATE INDEX idx_messages_post_id_created_at
ON messages(post_id, created_at DESC)
WHERE post_id IS NOT NULL;
