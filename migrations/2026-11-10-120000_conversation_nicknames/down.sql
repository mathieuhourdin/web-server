DROP TRIGGER hupo_nickname_events ON conversation_members;
DROP FUNCTION hupo_notify_conversation_nickname_change();
ALTER TABLE conversation_members DROP COLUMN nickname;
