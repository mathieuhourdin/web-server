-- Effective grants remain the read ACL; sources explain independent decisions.
-- audience_id on post_grants is retained as legacy migration metadata only.
CREATE TABLE post_grant_sources (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    post_grant_id UUID NOT NULL REFERENCES post_grants(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL CHECK (source_type IN ('MANUAL','AUDIENCE')),
    audience_id UUID REFERENCES audiences(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('ACTIVE','REVOKED')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CHECK ((source_type='AUDIENCE' AND audience_id IS NOT NULL)
        OR (source_type='MANUAL' AND audience_id IS NULL))
);
CREATE UNIQUE INDEX post_grant_sources_audience_unique
    ON post_grant_sources(post_grant_id,audience_id) WHERE source_type='AUDIENCE';
CREATE UNIQUE INDEX post_grant_sources_manual_unique
    ON post_grant_sources(post_grant_id) WHERE source_type='MANUAL';
CREATE INDEX post_grant_sources_audience_idx ON post_grant_sources(audience_id,post_grant_id);

-- Exactly one source per existing direct grant. Never infer additional access.
-- Inherited complement grants continue to follow source_grant_id instead.
INSERT INTO post_grant_sources(post_grant_id,source_type,audience_id,status,created_at,updated_at)
SELECT id, CASE WHEN audience_id IS NULL THEN 'MANUAL' ELSE 'AUDIENCE' END,
       audience_id,status,created_at,updated_at
FROM post_grants WHERE source_grant_id IS NULL;

CREATE FUNCTION seed_post_grant_source() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.source_grant_id IS NULL THEN
        INSERT INTO post_grant_sources(post_grant_id,source_type,audience_id,status)
        VALUES(NEW.id,CASE WHEN NEW.audience_id IS NULL THEN 'MANUAL' ELSE 'AUDIENCE' END,
            NEW.audience_id,NEW.status);
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER seed_post_grant_source AFTER INSERT ON post_grants
FOR EACH ROW EXECUTE FUNCTION seed_post_grant_source();

-- Individual revocation and existing block/unfollow paths revoke every source.
-- Audience revocation instead updates only its sources before reconciling grants.
CREATE FUNCTION revoke_post_grant_sources() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.source_grant_id IS NULL AND NEW.status='REVOKED' THEN
        UPDATE post_grant_sources SET status='REVOKED',updated_at=NOW()
        WHERE post_grant_id=NEW.id AND status='ACTIVE';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER revoke_post_grant_sources AFTER UPDATE OF status ON post_grants
FOR EACH ROW EXECUTE FUNCTION revoke_post_grant_sources();

-- An audience's eventual physical deletion must not leave stale effective ACLs.
CREATE FUNCTION reconcile_deleted_post_grant_source() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE publication UUID;
BEGIN
    SELECT post_id INTO publication FROM post_grants WHERE id=OLD.post_grant_id;
    IF publication IS NULL THEN RETURN OLD; END IF;
    PERFORM 1 FROM posts WHERE id=publication FOR UPDATE;
    UPDATE post_grants g SET status='REVOKED',updated_at=NOW()
    WHERE g.id=OLD.post_grant_id AND g.status='ACTIVE'
        AND NOT EXISTS (SELECT 1 FROM post_grant_sources s WHERE s.post_grant_id=g.id AND s.status='ACTIVE');
    RETURN OLD;
END;
$$;
CREATE TRIGGER reconcile_deleted_post_grant_source AFTER DELETE ON post_grant_sources
FOR EACH ROW EXECUTE FUNCTION reconcile_deleted_post_grant_source();

CREATE OR REPLACE FUNCTION grant_audience_post(aid UUID, publication UUID, reader UUID, restore BOOLEAN)
RETURNS BOOLEAN LANGUAGE plpgsql AS $$
DECLARE owner UUID; grant_id UUID; inherited UUID; was_active BOOLEAN; source_status TEXT;
BEGIN
    SELECT owner_user_id INTO owner FROM audiences WHERE id=aid AND archived_at IS NULL FOR UPDATE;
    IF owner IS NULL OR NOT EXISTS (SELECT 1 FROM post_audiences WHERE audience_id=aid AND post_id=publication)
        OR NOT EXISTS (SELECT 1 FROM audience_members WHERE audience_id=aid AND user_id=reader
            AND (status='ACTIVE' OR (restore AND status='SUGGESTED')))
        OR NOT EXISTS (SELECT 1 FROM posts p JOIN traces t ON t.id=p.source_trace_id
            WHERE p.id=publication AND p.user_id=owner AND t.trace_type='USER_TRACE') THEN RETURN FALSE; END IF;
    PERFORM 1 FROM posts WHERE id=publication FOR UPDATE;
    IF NOT trace_publication_eligible(publication,reader) AND NOT EXISTS (
        SELECT 1 FROM posts p JOIN traces t ON t.id=p.source_trace_id
        WHERE p.id=publication AND p.status='DRAFT' AND t.status IN ('DRAFT','FINALIZED') AND NOT t.is_encrypted
            AND (p.user_id=t.user_id OR EXISTS (SELECT 1 FROM trace_mentions m WHERE m.trace_id=t.id
                AND m.mentioned_user_id=p.user_id AND m.removed_at IS NULL AND m.allows_reshare))
            AND NOT EXISTS (SELECT 1 FROM user_blocks b WHERE
                (b.blocker_user_id=reader AND b.blocked_user_id IN (p.user_id,t.user_id))
                OR (b.blocked_user_id=reader AND b.blocker_user_id IN (p.user_id,t.user_id)))
    ) THEN RETURN FALSE; END IF;
    PERFORM 1 FROM relationships WHERE requester_user_id=reader AND target_user_id=owner
        AND relationship_type='FOLLOW' AND status='ACCEPTED' FOR SHARE;
    IF NOT FOUND THEN RETURN FALSE; END IF;
    SELECT id,source_grant_id,status='ACTIVE' INTO grant_id,inherited,was_active
        FROM post_grants WHERE post_id=publication AND grantee_user_id=reader FOR UPDATE;
    IF grant_id IS NULL THEN
        INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,access_level,status,audience_id)
        VALUES(uuid_generate_v4(),publication,owner,reader,'READ','ACTIVE',aid);
        RETURN TRUE;
    END IF;
    IF inherited IS NOT NULL THEN RETURN FALSE; END IF;
    SELECT status INTO source_status FROM post_grant_sources
        WHERE post_grant_id=grant_id AND audience_id=aid;
    -- Repeated attachment cannot restore an individually withdrawn source.
    IF source_status='REVOKED' AND NOT restore THEN RETURN FALSE; END IF;
    INSERT INTO post_grant_sources(post_grant_id,source_type,audience_id,status)
        VALUES(grant_id,'AUDIENCE',aid,'ACTIVE')
        ON CONFLICT(post_grant_id,audience_id) WHERE source_type='AUDIENCE'
        DO UPDATE SET status='ACTIVE',updated_at=NOW();
    UPDATE post_grants SET status='ACTIVE',updated_at=NOW() WHERE id=grant_id AND status<>'ACTIVE';
    RETURN NOT was_active;
END;
$$;

-- Return how many effective grants lost access, not how many origins changed.
CREATE FUNCTION revoke_audience_grant_sources(aid UUID, reader UUID, publication UUID)
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE changed BIGINT;
BEGIN
    -- Match the post-first grant lock order used by manual changes and inheritance.
    PERFORM 1 FROM posts p WHERE EXISTS (
        SELECT 1 FROM post_grants g JOIN post_grant_sources s ON s.post_grant_id=g.id
        WHERE g.post_id=p.id AND s.audience_id=aid AND s.status='ACTIVE'
            AND (reader IS NULL OR g.grantee_user_id=reader)
            AND (publication IS NULL OR g.post_id=publication)
    ) ORDER BY p.id FOR UPDATE;
    PERFORM 1 FROM post_grants g WHERE EXISTS (
        SELECT 1 FROM post_grant_sources s WHERE s.post_grant_id=g.id
            AND s.audience_id=aid AND s.status='ACTIVE'
    ) AND (reader IS NULL OR g.grantee_user_id=reader)
      AND (publication IS NULL OR g.post_id=publication)
    ORDER BY g.id FOR UPDATE;
    UPDATE post_grant_sources s SET status='REVOKED',updated_at=NOW()
    FROM post_grants g WHERE s.post_grant_id=g.id AND s.audience_id=aid AND s.status='ACTIVE'
        AND (reader IS NULL OR g.grantee_user_id=reader)
        AND (publication IS NULL OR g.post_id=publication);
    UPDATE post_grants g SET status='REVOKED',updated_at=NOW()
    WHERE g.source_grant_id IS NULL AND g.status='ACTIVE'
        AND (reader IS NULL OR g.grantee_user_id=reader)
        AND (publication IS NULL OR g.post_id=publication)
        AND EXISTS (SELECT 1 FROM post_grant_sources s WHERE s.post_grant_id=g.id AND s.audience_id=aid)
        AND NOT EXISTS (SELECT 1 FROM post_grant_sources s WHERE s.post_grant_id=g.id AND s.status='ACTIVE');
    GET DIAGNOSTICS changed = ROW_COUNT;
    RETURN changed;
END;
$$;
