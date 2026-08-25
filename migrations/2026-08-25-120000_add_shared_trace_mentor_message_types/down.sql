ALTER TABLE messages
DROP CONSTRAINT IF EXISTS messages_message_type_check;

ALTER TABLE messages
ADD CONSTRAINT messages_message_type_check
CHECK (
    message_type IN (
        'GENERAL',
        'MENTOR_FEEDBACK',
        'QUESTION',
        'MENTOR_REPLY',
        'TAROT_READING_REQUEST'
    )
);
