ALTER TABLE messages
DROP CONSTRAINT IF EXISTS messages_attachment_type_check;

ALTER TABLE messages
ADD CONSTRAINT messages_attachment_type_check
CHECK (
    attachment_type IS NULL
    OR attachment_type IN (
        'TAROT_READING',
        'SHARED_TRACE_TRANSLATION'
    )
);
