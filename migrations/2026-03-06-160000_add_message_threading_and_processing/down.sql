DROP INDEX IF EXISTS idx_messages_trace_created_at;
DROP INDEX IF EXISTS idx_messages_reply_to_message_id;

ALTER TABLE messages
DROP CONSTRAINT IF EXISTS messages_processing_state_check;

ALTER TABLE messages
DROP CONSTRAINT IF EXISTS messages_message_type_check;

ALTER TABLE messages
ADD CONSTRAINT messages_message_type_check
CHECK (message_type IN ('GENERAL', 'MENTOR_FEEDBACK'));

ALTER TABLE messages
DROP COLUMN processing_state,
DROP COLUMN reply_to_message_id;
