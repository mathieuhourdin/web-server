ALTER TABLE messages
DROP CONSTRAINT IF EXISTS messages_attachment_presence_check;

ALTER TABLE messages
DROP CONSTRAINT IF EXISTS messages_attachment_type_check;

ALTER TABLE messages
DROP COLUMN IF EXISTS attachment,
DROP COLUMN IF EXISTS attachment_type;
