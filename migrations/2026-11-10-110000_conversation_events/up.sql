-- Transient wake-ups, not an event log. PostgreSQL delivers NOTIFY at commit.
-- No titles, contents, tokens or personal profile fields enter this channel.
CREATE FUNCTION hupo_notify_message_change() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE kind text; cid uuid; mid uuid; pos bigint;
BEGIN
    IF TG_OP='DELETE' THEN
        kind := 'message_deleted'; cid := OLD.conversation_id; mid := OLD.id; pos := OLD.position;
    ELSE
        IF TG_OP='INSERT' THEN
            IF NEW.processing_state<>'PROCESSED' THEN RETURN NEW; END IF;
            kind := 'message_created';
        ELSE
            IF ROW(NEW.title,NEW.content,NEW.message_type,NEW.processing_state,NEW.attachment_type,
                   NEW.attachment,NEW.metadata,NEW.suggested_actions,NEW.trace_id,NEW.post_id,
                   NEW.journal_id,NEW.landscape_analysis_id,NEW.reply_to_message_id)
               IS NOT DISTINCT FROM
               ROW(OLD.title,OLD.content,OLD.message_type,OLD.processing_state,OLD.attachment_type,
                   OLD.attachment,OLD.metadata,OLD.suggested_actions,OLD.trace_id,OLD.post_id,
                   OLD.journal_id,OLD.landscape_analysis_id,OLD.reply_to_message_id) THEN RETURN NEW; END IF;
            IF NEW.processing_state='PROCESSED' AND OLD.processing_state<>'PROCESSED' THEN
                kind := 'message_created';
            ELSE
                kind := 'message_updated';
            END IF;
        END IF;
        cid := NEW.conversation_id; mid := NEW.id; pos := NEW.position;
    END IF;
    PERFORM pg_notify('hupo_conversation_events',json_build_object(
        'kind',kind,'conversation_id',cid,'message_id',mid,'position',pos)::text);
    RETURN NULL;
END $$;
CREATE TRIGGER hupo_message_events AFTER INSERT OR DELETE OR UPDATE OF
    title,content,message_type,processing_state,attachment_type,attachment,metadata,
    suggested_actions,trace_id,post_id,journal_id,landscape_analysis_id,reply_to_message_id
ON messages FOR EACH ROW EXECUTE FUNCTION hupo_notify_message_change();

CREATE FUNCTION hupo_notify_reaction_change() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE mid uuid; cid uuid; pos bigint;
BEGIN
    IF TG_OP='UPDATE' AND NEW.emoji IS NOT DISTINCT FROM OLD.emoji THEN RETURN NULL; END IF;
    IF TG_OP='DELETE' THEN mid := OLD.message_id; ELSE mid := NEW.message_id; END IF;
    SELECT conversation_id,position INTO cid,pos FROM messages WHERE id=mid;
    IF cid IS NOT NULL THEN
        PERFORM pg_notify('hupo_conversation_events',json_build_object(
            'kind','reaction_updated','conversation_id',cid,'message_id',mid,'position',pos)::text);
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER hupo_reaction_events AFTER INSERT OR UPDATE OF emoji OR DELETE
ON message_reactions FOR EACH ROW EXECUTE FUNCTION hupo_notify_reaction_change();

CREATE FUNCTION hupo_notify_membership_change() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE cid uuid; uid uuid;
BEGIN
    IF TG_OP='UPDATE' AND ROW(NEW.role,NEW.joined_at,NEW.left_at,NEW.history_start_position,NEW.last_read_position,NEW.muted_at)
       IS NOT DISTINCT FROM ROW(OLD.role,OLD.joined_at,OLD.left_at,OLD.history_start_position,OLD.last_read_position,OLD.muted_at)
       THEN RETURN NULL; END IF;
    IF TG_OP='DELETE' THEN cid := OLD.conversation_id; uid := OLD.user_id;
    ELSE cid := NEW.conversation_id; uid := NEW.user_id; END IF;
    -- Read/mute cursor changes are private to the member. Actual admissions,
    -- departures and role changes also invalidate other members' summaries.
    IF TG_OP='UPDATE' AND ROW(NEW.role,NEW.joined_at,NEW.left_at,NEW.history_start_position)
        IS NOT DISTINCT FROM ROW(OLD.role,OLD.joined_at,OLD.left_at,OLD.history_start_position) THEN
        PERFORM pg_notify('hupo_conversation_events',json_build_object(
            'kind','member_state_changed','conversation_id',cid,'user_id',uid)::text);
    ELSE
        PERFORM pg_notify('hupo_conversation_events',json_build_object(
            'kind','membership_changed','conversation_id',cid,'user_id',uid)::text);
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER hupo_membership_events AFTER INSERT OR UPDATE OR DELETE
ON conversation_members FOR EACH ROW EXECUTE FUNCTION hupo_notify_membership_change();

CREATE FUNCTION hupo_notify_conversation_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.title IS DISTINCT FROM OLD.title THEN
        PERFORM pg_notify('hupo_conversation_events',json_build_object(
            'kind','conversation_changed','conversation_id',NEW.id)::text);
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER hupo_conversation_events AFTER UPDATE OF title ON conversations
FOR EACH ROW EXECUTE FUNCTION hupo_notify_conversation_change();

CREATE FUNCTION hupo_notify_conversation_access_change() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE a uuid; b uuid;
BEGIN
    IF TG_OP='DELETE' THEN a := OLD.blocker_user_id; b := OLD.blocked_user_id;
    ELSE a := NEW.blocker_user_id; b := NEW.blocked_user_id; END IF;
    PERFORM pg_notify('hupo_conversation_events',json_build_object(
        'kind','access_changed','user_id',a,'other_user_id',b)::text);
    RETURN NULL;
END $$;
CREATE TRIGGER hupo_conversation_access_events AFTER INSERT OR DELETE ON user_blocks
FOR EACH ROW EXECUTE FUNCTION hupo_notify_conversation_access_change();
