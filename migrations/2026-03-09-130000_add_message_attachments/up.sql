ALTER TABLE messages
ADD COLUMN attachment_type TEXT NULL,
ADD COLUMN attachment JSONB NULL;

ALTER TABLE messages
ADD CONSTRAINT messages_attachment_type_check
CHECK (
    attachment_type IS NULL
    OR attachment_type IN ('TAROT_READING')
);

ALTER TABLE messages
ADD CONSTRAINT messages_attachment_presence_check
CHECK (
    (attachment_type IS NULL AND attachment IS NULL)
    OR (attachment_type IS NOT NULL AND attachment IS NOT NULL)
);
