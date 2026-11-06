-- A downgrade cannot represent group messages or discard saved provenance.
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM conversations WHERE kind='GROUP') OR EXISTS(SELECT 1 FROM trace_message_sources) THEN
        RAISE EXCEPTION 'Cannot downgrade conversations while group data or trace provenance exists';
    END IF;
END $$;
DROP TABLE trace_message_sources;
DROP TRIGGER conversation_members_repair_admin ON conversation_members;
DROP FUNCTION conversation_repair_admin_after_delete();
DROP TRIGGER messages_protect_conversation ON messages;
DROP FUNCTION protect_message_conversation();
DROP TRIGGER messages_assign_conversation ON messages;
DROP FUNCTION assign_message_conversation();
DROP FUNCTION ensure_direct_conversation(uuid,uuid);
DROP FUNCTION conversation_message_unread(uuid,uuid);
DROP FUNCTION conversation_message_readable(uuid,uuid);
DROP FUNCTION conversation_readable(uuid,uuid);
ALTER TABLE messages DROP COLUMN position;
ALTER TABLE messages DROP COLUMN conversation_id;
ALTER TABLE messages ALTER COLUMN recipient_user_id SET NOT NULL;
DROP TABLE conversation_members;
DROP TABLE conversations;
