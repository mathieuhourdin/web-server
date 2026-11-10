ALTER TABLE conversation_members ADD COLUMN nickname text
    CHECK (nickname IS NULL OR char_length(nickname) BETWEEN 1 AND 80);

CREATE FUNCTION hupo_notify_conversation_nickname_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.nickname IS DISTINCT FROM OLD.nickname THEN
        -- The nickname itself stays out of PostgreSQL's notification payload.
        PERFORM pg_notify('hupo_conversation_events',json_build_object(
            'kind','membership_changed','conversation_id',NEW.conversation_id,'user_id',NEW.user_id)::text);
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER hupo_nickname_events AFTER UPDATE OF nickname ON conversation_members
FOR EACH ROW EXECUTE FUNCTION hupo_notify_conversation_nickname_change();
