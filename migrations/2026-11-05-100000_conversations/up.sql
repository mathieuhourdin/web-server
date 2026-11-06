CREATE TABLE conversations (
    id uuid PRIMARY KEY DEFAULT uuid_generate_v4(),
    kind text NOT NULL CHECK (kind IN ('DIRECT','GROUP')),
    title text CHECK (char_length(title)<=200),
    created_by_user_id uuid REFERENCES users(id) ON DELETE SET NULL,
    direct_user_low_id uuid REFERENCES users(id) ON DELETE CASCADE,
    direct_user_high_id uuid REFERENCES users(id) ON DELETE CASCADE,
    last_message_position bigint NOT NULL DEFAULT 0 CHECK (last_message_position>=0),
    created_at timestamp NOT NULL DEFAULT NOW(),
    updated_at timestamp NOT NULL DEFAULT NOW(),
    CHECK ((kind='DIRECT' AND direct_user_low_id IS NOT NULL AND direct_user_high_id IS NOT NULL
                AND direct_user_low_id<=direct_user_high_id)
        OR (kind='GROUP' AND direct_user_low_id IS NULL AND direct_user_high_id IS NULL))
);
CREATE UNIQUE INDEX conversations_direct_pair ON conversations(direct_user_low_id,direct_user_high_id) WHERE kind='DIRECT';
CREATE TABLE conversation_members (
    conversation_id uuid NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role text NOT NULL DEFAULT 'MEMBER' CHECK (role IN ('ADMIN','MEMBER')),
    joined_at timestamp NOT NULL DEFAULT NOW(),
    left_at timestamp,
    history_start_position bigint NOT NULL DEFAULT 1 CHECK (history_start_position>=1),
    last_read_position bigint NOT NULL DEFAULT 0 CHECK (last_read_position>=0),
    muted_at timestamp,
    PRIMARY KEY (conversation_id,user_id)
);
CREATE INDEX conversation_members_active_user ON conversation_members(user_id,conversation_id) WHERE left_at IS NULL;
ALTER TABLE messages ALTER COLUMN recipient_user_id DROP NOT NULL;
ALTER TABLE messages ADD COLUMN conversation_id uuid REFERENCES conversations(id) ON DELETE CASCADE;
ALTER TABLE messages ADD COLUMN position bigint;

INSERT INTO conversations(kind,direct_user_low_id,direct_user_high_id,created_at,updated_at)
SELECT 'DIRECT',LEAST(sender_user_id,recipient_user_id),GREATEST(sender_user_id,recipient_user_id),MIN(created_at),MAX(updated_at)
FROM messages GROUP BY 2,3;
UPDATE messages m SET conversation_id=c.id FROM conversations c
WHERE c.kind='DIRECT' AND c.direct_user_low_id=LEAST(m.sender_user_id,m.recipient_user_id)
    AND c.direct_user_high_id=GREATEST(m.sender_user_id,m.recipient_user_id);
WITH ranked AS (SELECT id,ROW_NUMBER() OVER (PARTITION BY conversation_id ORDER BY created_at,id) AS position FROM messages)
UPDATE messages m SET position=r.position FROM ranked r WHERE r.id=m.id;
UPDATE conversations c SET last_message_position=(SELECT MAX(position) FROM messages WHERE conversation_id=c.id);
INSERT INTO conversation_members(conversation_id,user_id,joined_at,last_read_position)
SELECT c.id,u.user_id,c.created_at,COALESCE((SELECT MIN(m.position)-1 FROM messages m
    WHERE m.conversation_id=c.id AND m.recipient_user_id=u.user_id AND m.seen_at IS NULL
        AND m.processing_state IN ('PROCESSED','PENDING','RUNNING')),c.last_message_position)
FROM conversations c CROSS JOIN LATERAL (SELECT c.direct_user_low_id AS user_id UNION SELECT c.direct_user_high_id) u;
ALTER TABLE messages ALTER COLUMN conversation_id SET NOT NULL;
ALTER TABLE messages ALTER COLUMN position SET NOT NULL;
ALTER TABLE messages ADD CONSTRAINT messages_conversation_position_unique UNIQUE(conversation_id,position);
ALTER TABLE messages ADD CONSTRAINT messages_positive_position CHECK(position>0);

CREATE FUNCTION conversation_readable(conversation uuid,viewer uuid) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM conversations c JOIN conversation_members cm ON cm.conversation_id=c.id
        WHERE c.id=conversation AND cm.user_id=viewer AND cm.left_at IS NULL
        AND (c.kind='GROUP' OR automation_same_sandbox(c.direct_user_low_id,c.direct_user_high_id)
            OR EXISTS(SELECT 1 FROM user_roles ur WHERE ur.user_id IN(c.direct_user_low_id,c.direct_user_high_id) AND ur.role='MENTOR'))
        AND (c.kind='DIRECT' OR NOT EXISTS(SELECT 1 FROM conversation_members peer WHERE peer.conversation_id=c.id
            AND peer.left_at IS NULL AND NOT automation_same_sandbox(peer.user_id,viewer)))
        AND (c.kind='GROUP' OR NOT EXISTS(SELECT 1 FROM user_blocks b
            WHERE (b.blocker_user_id=c.direct_user_low_id AND b.blocked_user_id=c.direct_user_high_id)
               OR (b.blocker_user_id=c.direct_user_high_id AND b.blocked_user_id=c.direct_user_low_id))))
$$;
CREATE FUNCTION conversation_message_readable(message uuid,viewer uuid) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM messages m JOIN conversation_members cm ON cm.conversation_id=m.conversation_id
        WHERE m.id=message AND cm.user_id=viewer AND cm.left_at IS NULL
          AND m.position>=cm.history_start_position AND conversation_readable(m.conversation_id,viewer))
$$;
CREATE FUNCTION ensure_direct_conversation(sender uuid,recipient uuid) RETURNS uuid
LANGUAGE plpgsql AS $$
DECLARE cid uuid;
BEGIN
    INSERT INTO conversations(kind,direct_user_low_id,direct_user_high_id,created_by_user_id)
    VALUES('DIRECT',LEAST(sender,recipient),GREATEST(sender,recipient),sender)
    ON CONFLICT(direct_user_low_id,direct_user_high_id) WHERE kind='DIRECT'
    DO UPDATE SET kind=EXCLUDED.kind RETURNING id INTO cid;
    INSERT INTO conversation_members(conversation_id,user_id)
    SELECT cid,sender UNION SELECT cid,recipient ON CONFLICT DO NOTHING;
    RETURN cid;
END $$;
CREATE FUNCTION conversation_message_unread(message uuid,viewer uuid) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM messages m JOIN conversation_members cm ON cm.conversation_id=m.conversation_id
        WHERE m.id=message AND cm.user_id=viewer AND m.sender_user_id<>viewer
          AND m.processing_state='PROCESSED' AND m.position>cm.last_read_position
          AND (m.recipient_user_id IS NULL OR m.seen_at IS NULL)
          AND conversation_message_readable(m.id,viewer))
$$;
CREATE FUNCTION assign_message_conversation() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE c conversations; target messages;
BEGIN
    IF NEW.conversation_id IS NULL THEN
        IF NEW.recipient_user_id IS NULL THEN RAISE EXCEPTION 'A message requires a conversation or direct recipient'; END IF;
        NEW.conversation_id:=ensure_direct_conversation(NEW.sender_user_id,NEW.recipient_user_id);
    END IF;
    SELECT * INTO STRICT c FROM conversations WHERE id=NEW.conversation_id FOR UPDATE;
    IF c.kind='GROUP' AND NEW.recipient_user_id IS NOT NULL THEN RAISE EXCEPTION 'Group messages have no direct recipient'; END IF;
    IF c.kind='DIRECT' AND (NEW.recipient_user_id IS NULL OR LEAST(NEW.sender_user_id,NEW.recipient_user_id)<>c.direct_user_low_id
        OR GREATEST(NEW.sender_user_id,NEW.recipient_user_id)<>c.direct_user_high_id) THEN
        RAISE EXCEPTION 'Direct message participants do not match the conversation';
    END IF;
    IF NOT EXISTS(SELECT 1 FROM conversation_members WHERE conversation_id=c.id AND user_id=NEW.sender_user_id AND left_at IS NULL) THEN
        RAISE EXCEPTION 'Message sender is not an active conversation member';
    END IF;
    IF NEW.reply_to_message_id IS NOT NULL THEN
        SELECT * INTO STRICT target FROM messages WHERE id=NEW.reply_to_message_id;
        IF target.conversation_id<>c.id OR target.processing_state<>'PROCESSED'
            OR NOT conversation_message_readable(target.id,NEW.sender_user_id) THEN RAISE EXCEPTION 'Reply target is not visible in this conversation'; END IF;
    END IF;
    UPDATE conversations SET last_message_position=last_message_position+1,updated_at=NOW() WHERE id=c.id
        RETURNING last_message_position INTO NEW.position;
    RETURN NEW;
END $$;
CREATE TRIGGER messages_assign_conversation BEFORE INSERT ON messages FOR EACH ROW EXECUTE FUNCTION assign_message_conversation();
CREATE FUNCTION protect_message_conversation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.conversation_id,NEW.position,NEW.sender_user_id,NEW.recipient_user_id)
        IS DISTINCT FROM (OLD.conversation_id,OLD.position,OLD.sender_user_id,OLD.recipient_user_id) THEN
        RAISE EXCEPTION 'Message conversation and participants are immutable';
    END IF;
    IF NEW.reply_to_message_id IS DISTINCT FROM OLD.reply_to_message_id AND NEW.reply_to_message_id IS NOT NULL
        AND NOT EXISTS(SELECT 1 FROM messages m WHERE m.id=NEW.reply_to_message_id AND m.conversation_id=NEW.conversation_id
            AND m.processing_state='PROCESSED' AND conversation_message_readable(m.id,NEW.sender_user_id)) THEN
        RAISE EXCEPTION 'Reply target is not visible in this conversation';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER messages_protect_conversation BEFORE UPDATE ON messages FOR EACH ROW EXECUTE FUNCTION protect_message_conversation();

CREATE TABLE trace_message_sources (
    trace_id uuid NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    message_id uuid NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    created_at timestamp NOT NULL DEFAULT NOW(),
    PRIMARY KEY(trace_id,message_id)
);
CREATE INDEX trace_message_sources_message ON trace_message_sources(message_id);

-- Account deletion must not strand remaining group members without an admin.
CREATE FUNCTION conversation_repair_admin_after_delete() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM 1 FROM conversations WHERE id=OLD.conversation_id AND kind='GROUP' FOR UPDATE;
    IF FOUND AND NOT EXISTS(SELECT 1 FROM conversation_members WHERE conversation_id=OLD.conversation_id AND left_at IS NULL AND role='ADMIN') THEN
        UPDATE conversation_members SET role='ADMIN' WHERE (conversation_id,user_id)=(
            SELECT conversation_id,user_id FROM conversation_members WHERE conversation_id=OLD.conversation_id AND left_at IS NULL
            ORDER BY joined_at,user_id LIMIT 1);
    END IF;
    RETURN OLD;
END $$;
CREATE TRIGGER conversation_members_repair_admin AFTER DELETE ON conversation_members
    FOR EACH ROW EXECUTE FUNCTION conversation_repair_admin_after_delete();
