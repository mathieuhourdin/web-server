-- Refuse to silently discard sent-media metadata on rollback.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM message_images WHERE message_id IS NOT NULL) THEN
        RAISE EXCEPTION 'Cannot roll back message images while sent attachments exist';
    END IF;
END $$;
DROP TABLE message_images;
