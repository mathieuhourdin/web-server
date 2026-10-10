CREATE TABLE audiences (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 100),
    source_journal_id UUID UNIQUE REFERENCES journals(id) ON DELETE SET NULL,
    archived_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE INDEX audiences_owner_idx ON audiences(owner_user_id, created_at, id);
CREATE TABLE audience_members (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    audience_id UUID NOT NULL REFERENCES audiences(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','SUGGESTED','DISABLED','DISMISSED','REVOKED')),
    future_access_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    history_cutoff_at TIMESTAMP NOT NULL DEFAULT NOW(),
    history_review_state TEXT NOT NULL DEFAULT 'UNREVIEWED' CHECK (history_review_state IN ('NOT_STARTED','UNREVIEWED','REVIEWED')),
    history_decision TEXT CHECK (history_decision IN ('NONE','ALL_NORMAL','ALL_INCLUDING_SENSITIVE','USER_SELECTED')),
    history_reviewed_at TIMESTAMP,
    legacy_policy_id UUID UNIQUE REFERENCES journal_sharing_policies(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(audience_id, user_id)
);
CREATE INDEX audience_members_user_idx ON audience_members(user_id);
CREATE TABLE post_audiences (
    post_id UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    audience_id UUID NOT NULL REFERENCES audiences(id) ON DELETE CASCADE,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY(post_id, audience_id)
);
CREATE INDEX post_audiences_audience_idx ON post_audiences(audience_id, post_id);
CREATE TABLE journal_default_audiences (
    journal_id UUID NOT NULL REFERENCES journals(id) ON DELETE CASCADE,
    audience_id UUID NOT NULL REFERENCES audiences(id) ON DELETE CASCADE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY(journal_id, audience_id)
);
ALTER TABLE post_grants ADD COLUMN audience_id UUID REFERENCES audiences(id) ON DELETE SET NULL;
CREATE INDEX post_grants_audience_reader_idx ON post_grants(audience_id, grantee_user_id)
    WHERE audience_id IS NOT NULL;

-- Adopt existing sharing without changing grant count or access status.
INSERT INTO audiences(owner_user_id, name, source_journal_id)
SELECT j.user_id, LEFT(COALESCE(NULLIF(btrim(j.title), ''), 'Journal'),100), j.id
FROM journals j WHERE EXISTS (SELECT 1 FROM journal_sharing_policies s WHERE s.journal_id=j.id);
INSERT INTO audience_members(id,audience_id,user_id,status,future_access_enabled,
    history_cutoff_at,history_review_state,history_decision,history_reviewed_at,legacy_policy_id,created_at,updated_at)
SELECT s.id,a.id,s.grantee_user_id,s.status,s.default_future_access_enabled,
    NOW(),s.history_review_state,s.history_decision,s.history_reviewed_at,s.id,s.created_at,s.updated_at
FROM journal_sharing_policies s JOIN audiences a ON a.source_journal_id=s.journal_id;
INSERT INTO journal_default_audiences(journal_id,audience_id)
SELECT source_journal_id,id FROM audiences WHERE source_journal_id IS NOT NULL;
INSERT INTO post_audiences(post_id,audience_id,is_default)
SELECT p.id,a.id,TRUE FROM audiences a JOIN traces t ON t.journal_id=a.source_journal_id
JOIN posts p ON p.source_trace_id=t.id AND p.user_id=a.owner_user_id
WHERE t.trace_type='USER_TRACE';
-- Attribution must not repair/materialize inherited grants as a side effect.
ALTER TABLE post_grants DISABLE TRIGGER sync_complement_grants;
UPDATE post_grants g SET audience_id=a.id
FROM posts p,traces t,audiences a,audience_members m
WHERE g.post_id=p.id AND p.source_trace_id=t.id AND p.user_id=a.owner_user_id
  AND t.journal_id=a.source_journal_id AND m.audience_id=a.id AND m.user_id=g.grantee_user_id
  AND g.owner_user_id=a.owner_user_id AND g.grantee_scope IS NULL AND g.source_grant_id IS NULL;

-- A grant is first-writer-wins. Only explicit sharing can reactivate revoked rows.
CREATE FUNCTION grant_audience_post(aid UUID, publication UUID, reader UUID, restore BOOLEAN)
RETURNS BOOLEAN LANGUAGE plpgsql AS $$
DECLARE owner UUID; changed UUID;
BEGIN
    SELECT owner_user_id INTO owner FROM audiences WHERE id=aid AND archived_at IS NULL FOR UPDATE;
    IF owner IS NULL OR NOT EXISTS (SELECT 1 FROM post_audiences WHERE audience_id=aid AND post_id=publication)
        OR NOT EXISTS (SELECT 1 FROM audience_members WHERE audience_id=aid AND user_id=reader
            AND (status='ACTIVE' OR (restore AND status='SUGGESTED')))
        OR NOT EXISTS (SELECT 1 FROM posts p JOIN traces t ON t.id=p.source_trace_id
            WHERE p.id=publication AND p.user_id=owner AND t.trace_type='USER_TRACE')
        OR NOT trace_publication_eligible(publication,reader) THEN RETURN FALSE; END IF;
    PERFORM 1 FROM relationships WHERE requester_user_id=reader AND target_user_id=owner
        AND relationship_type='FOLLOW' AND status='ACCEPTED' FOR SHARE;
    IF NOT FOUND THEN RETURN FALSE; END IF;
    INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,access_level,status,audience_id)
    VALUES(uuid_generate_v4(),publication,owner,reader,'READ','ACTIVE',aid)
    ON CONFLICT(post_id,grantee_user_id) WHERE grantee_user_id IS NOT NULL
    DO UPDATE SET status='ACTIVE', updated_at=NOW()
    WHERE post_grants.status <> 'ACTIVE' AND restore AND post_grants.source_grant_id IS NULL
    RETURNING id INTO changed;
    RETURN changed IS NOT NULL;
END;
$$;

-- Snapshot journal defaults on post creation; grant only when it is published.
-- No periodic reconciliation: that would undo explicit history revocation.
CREATE FUNCTION materialize_post_audiences() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE source traces%ROWTYPE; aid UUID; member UUID;
BEGIN
    IF NEW.source_trace_id IS NULL OR NEW.audience_source_post_id IS NOT NULL THEN RETURN NEW; END IF;
    SELECT * INTO source FROM traces WHERE id=NEW.source_trace_id;
    IF source.trace_type <> 'USER_TRACE' THEN RETURN NEW; END IF;
    IF NEW.audience_role<>'DEFAULT' THEN
        DELETE FROM post_audiences WHERE post_id=NEW.id AND is_default;
    END IF;
    IF TG_OP='INSERT' AND NEW.user_id=source.user_id AND NEW.audience_role='DEFAULT'
        AND source.sharing_sensitivity='NORMAL' THEN
        INSERT INTO post_audiences(post_id,audience_id,is_default)
        SELECT NEW.id,d.audience_id,TRUE FROM journal_default_audiences d
        JOIN audiences a ON a.id=d.audience_id AND a.archived_at IS NULL AND a.owner_user_id=NEW.user_id
        JOIN journals j ON j.id=d.journal_id AND j.status='ACTIVE' AND NOT j.is_encrypted
        WHERE d.journal_id=source.journal_id ON CONFLICT DO NOTHING;
    END IF;
    IF NEW.status <> 'PUBLISHED' THEN RETURN NEW; END IF;
    IF TG_OP='UPDATE' AND OLD.status='PUBLISHED' THEN RETURN NEW; END IF;
    FOR aid IN SELECT audience_id FROM post_audiences WHERE post_id=NEW.id
        AND (NOT is_default OR (NEW.audience_role='DEFAULT' AND source.sharing_sensitivity='NORMAL'))
        ORDER BY audience_id LOOP
        PERFORM 1 FROM audiences WHERE id=aid FOR UPDATE;
        FOR member IN SELECT user_id FROM audience_members WHERE audience_id=aid
            AND status='ACTIVE' AND future_access_enabled ORDER BY user_id LOOP
            PERFORM grant_audience_post(aid,NEW.id,member,FALSE);
        END LOOP;
    END LOOP;
    RETURN NEW;
END;
$$;
CREATE TRIGGER materialize_post_audiences AFTER INSERT OR UPDATE OF status,audience_role ON posts
FOR EACH ROW EXECUTE FUNCTION materialize_post_audiences();

CREATE FUNCTION validate_audience_association() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE owner UUID;
BEGIN
    SELECT owner_user_id INTO owner FROM audiences WHERE id=NEW.audience_id;
    IF TG_TABLE_NAME='post_audiences' THEN
        IF NOT EXISTS (SELECT 1 FROM posts p JOIN traces t ON t.id=p.source_trace_id
            WHERE p.id=NEW.post_id AND p.user_id=owner AND t.trace_type='USER_TRACE') THEN
            RAISE EXCEPTION 'Audience must belong to the trace publisher' USING ERRCODE='23514';
        END IF;
    ELSE
        IF NOT EXISTS (SELECT 1 FROM journals WHERE id=NEW.journal_id AND user_id=owner AND NOT is_encrypted) THEN
            RAISE EXCEPTION 'Default audience must belong to the journal owner' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER validate_post_audience BEFORE INSERT OR UPDATE ON post_audiences
FOR EACH ROW EXECUTE FUNCTION validate_audience_association();
CREATE TRIGGER validate_journal_default_audience BEFORE INSERT OR UPDATE ON journal_default_audiences
FOR EACH ROW EXECUTE FUNCTION validate_audience_association();

-- Preserve one-way inherited-complement synchronization, including grant origin.
CREATE OR REPLACE FUNCTION copy_complement_grants() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.audience_source_post_id IS NOT NULL THEN
        INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,grantee_scope,access_level,status,source_grant_id,audience_id)
        SELECT uuid_generate_v4(),NEW.id,NEW.user_id,g.grantee_user_id,g.grantee_scope,g.access_level,g.status,g.id,g.audience_id
        FROM post_grants g WHERE g.post_id=NEW.audience_source_post_id
        ON CONFLICT(post_id,source_grant_id) WHERE source_grant_id IS NOT NULL
        DO UPDATE SET status=EXCLUDED.status,access_level=EXCLUDED.access_level,audience_id=EXCLUDED.audience_id,updated_at=NOW();
    END IF;
    RETURN NEW;
END;
$$;
CREATE OR REPLACE FUNCTION sync_complement_grants() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE publication UUID;
BEGIN
    publication := CASE WHEN TG_OP='DELETE' THEN OLD.post_id ELSE NEW.post_id END;
    PERFORM 1 FROM posts WHERE id=publication FOR UPDATE;
    IF TG_OP='DELETE' THEN RETURN OLD; END IF;
    IF NEW.source_grant_id IS NOT NULL THEN RETURN NEW; END IF;
    INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,grantee_scope,access_level,status,source_grant_id,audience_id)
    SELECT uuid_generate_v4(),p.id,p.user_id,NEW.grantee_user_id,NEW.grantee_scope,NEW.access_level,NEW.status,NEW.id,NEW.audience_id
    FROM posts p WHERE p.audience_source_post_id=publication
    ON CONFLICT(post_id,source_grant_id) WHERE source_grant_id IS NOT NULL
    DO UPDATE SET grantee_user_id=EXCLUDED.grantee_user_id,grantee_scope=EXCLUDED.grantee_scope,
        access_level=EXCLUDED.access_level,status=EXCLUDED.status,audience_id=EXCLUDED.audience_id,updated_at=NOW();
    RETURN NEW;
END;
$$;
UPDATE post_grants child SET audience_id=parent.audience_id
FROM post_grants parent WHERE child.source_grant_id=parent.id AND child.audience_id IS DISTINCT FROM parent.audience_id;
ALTER TABLE post_grants ENABLE TRIGGER sync_complement_grants;

-- Legacy journal-policy routes remain usable while clients move to audiences.
CREATE FUNCTION mirror_journal_policy_audience() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE aid UUID; created UUID;
BEGIN
    IF pg_trigger_depth()>1 THEN RETURN NEW; END IF;
    INSERT INTO audiences(owner_user_id,name,source_journal_id)
    SELECT user_id,LEFT(COALESCE(NULLIF(btrim(title),''),'Journal'),100),id FROM journals WHERE id=NEW.journal_id
    ON CONFLICT(source_journal_id) DO NOTHING RETURNING id INTO created;
    SELECT id INTO aid FROM audiences WHERE source_journal_id=NEW.journal_id;
    IF created IS NOT NULL THEN
        INSERT INTO journal_default_audiences(journal_id,audience_id) VALUES(NEW.journal_id,aid) ON CONFLICT DO NOTHING;
        INSERT INTO post_audiences(post_id,audience_id,is_default)
        SELECT p.id,aid,TRUE FROM posts p JOIN traces t ON t.id=p.source_trace_id
        WHERE t.journal_id=NEW.journal_id AND p.user_id=NEW.owner_user_id AND t.trace_type='USER_TRACE' ON CONFLICT DO NOTHING;
    END IF;
    INSERT INTO audience_members(id,audience_id,user_id,status,future_access_enabled,history_review_state,
        history_decision,history_reviewed_at,legacy_policy_id)
    VALUES(NEW.id,aid,NEW.grantee_user_id,NEW.status,NEW.default_future_access_enabled,NEW.history_review_state,
        NEW.history_decision,NEW.history_reviewed_at,NEW.id)
    ON CONFLICT(audience_id,user_id) DO UPDATE SET
        history_cutoff_at=CASE WHEN audience_members.legacy_policy_id IS NOT NULL
            AND audience_members.history_review_state='REVIEWED' AND EXCLUDED.history_review_state='UNREVIEWED'
            THEN NOW() ELSE audience_members.history_cutoff_at END,
        status=CASE WHEN audience_members.legacy_policy_id IS NULL THEN audience_members.status ELSE EXCLUDED.status END,
        future_access_enabled=CASE WHEN audience_members.legacy_policy_id IS NULL THEN audience_members.future_access_enabled ELSE EXCLUDED.future_access_enabled END,
        history_review_state=CASE WHEN audience_members.legacy_policy_id IS NULL THEN audience_members.history_review_state ELSE EXCLUDED.history_review_state END,
        history_decision=CASE WHEN audience_members.legacy_policy_id IS NULL THEN audience_members.history_decision ELSE EXCLUDED.history_decision END,
        history_reviewed_at=CASE WHEN audience_members.legacy_policy_id IS NULL THEN audience_members.history_reviewed_at ELSE EXCLUDED.history_reviewed_at END,
        legacy_policy_id=EXCLUDED.legacy_policy_id,updated_at=NOW();
    UPDATE journal_sharing_policies s SET status=m.status, default_future_access_enabled=m.future_access_enabled,
        history_review_state=m.history_review_state,history_decision=m.history_decision,history_reviewed_at=m.history_reviewed_at
    FROM audience_members m WHERE m.legacy_policy_id=s.id AND s.id=NEW.id
        AND (s.status,s.default_future_access_enabled,s.history_review_state,s.history_decision,s.history_reviewed_at)
            IS DISTINCT FROM (m.status,m.future_access_enabled,m.history_review_state,m.history_decision,m.history_reviewed_at);
    RETURN NEW;
END;
$$;
CREATE FUNCTION mirror_audience_member_policy() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF pg_trigger_depth()>1 OR NEW.legacy_policy_id IS NULL THEN RETURN NEW; END IF;
    UPDATE journal_sharing_policies SET status=NEW.status, default_future_access_enabled=NEW.future_access_enabled,
        history_review_state=NEW.history_review_state,history_decision=NEW.history_decision,
        history_reviewed_at=NEW.history_reviewed_at,updated_at=NOW() WHERE id=NEW.legacy_policy_id;
    RETURN NEW;
END;
$$;
CREATE TRIGGER mirror_journal_policy_audience AFTER INSERT OR UPDATE ON journal_sharing_policies
FOR EACH ROW EXECUTE FUNCTION mirror_journal_policy_audience();
CREATE TRIGGER mirror_audience_member_policy AFTER UPDATE ON audience_members
FOR EACH ROW EXECUTE FUNCTION mirror_audience_member_policy();

CREATE FUNCTION preserve_active_grant_origin() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.source_grant_id IS NULL
        AND OLD.audience_id IS DISTINCT FROM NEW.audience_id THEN
        -- Account deletion may cascade through audiences before their grants.
        IF OLD.audience_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM audiences WHERE id=OLD.audience_id) THEN
            RETURN NEW;
        END IF;
        RAISE EXCEPTION 'Grant origin cannot be overwritten' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER preserve_active_grant_origin BEFORE UPDATE OF audience_id ON post_grants
FOR EACH ROW EXECUTE FUNCTION preserve_active_grant_origin();
