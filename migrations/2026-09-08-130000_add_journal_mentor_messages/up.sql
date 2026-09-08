ALTER TABLE messages
ADD COLUMN journal_id UUID REFERENCES journals(id) ON DELETE CASCADE;

CREATE INDEX messages_journal_conversation_idx
ON messages (journal_id, sender_user_id, recipient_user_id, created_at DESC)
WHERE journal_id IS NOT NULL;

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
        'TAROT_READING_REQUEST',
        'SHARED_TRACE_EXPLANATION_REQUEST',
        'SHARED_TRACE_TRANSLATION_REQUEST',
        'JOURNAL_FEEDBACK_REQUEST'
    )
);
