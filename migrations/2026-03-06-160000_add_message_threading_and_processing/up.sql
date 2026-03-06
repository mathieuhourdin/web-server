ALTER TABLE messages
ADD COLUMN reply_to_message_id UUID NULL REFERENCES messages(id) ON DELETE SET NULL,
ADD COLUMN processing_state TEXT;

UPDATE messages
SET processing_state = 'PROCESSED'
WHERE processing_state IS NULL;

ALTER TABLE messages
ALTER COLUMN processing_state SET NOT NULL;

ALTER TABLE messages
ADD CONSTRAINT messages_processing_state_check
CHECK (processing_state IN ('PENDING', 'RUNNING', 'PROCESSED', 'FAILED'));

ALTER TABLE messages
DROP CONSTRAINT messages_message_type_check;

ALTER TABLE messages
ADD CONSTRAINT messages_message_type_check
CHECK (message_type IN ('GENERAL', 'MENTOR_FEEDBACK', 'QUESTION', 'MENTOR_REPLY'));

CREATE INDEX idx_messages_reply_to_message_id
    ON messages(reply_to_message_id);

CREATE INDEX idx_messages_trace_created_at
    ON messages(trace_id, created_at DESC);
