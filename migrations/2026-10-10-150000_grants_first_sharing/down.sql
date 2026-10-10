-- Coordinated rollback only; this does not undo grants edited after cutover.
ALTER TABLE posts ALTER COLUMN public_link_eligible DROP DEFAULT;
ALTER TABLE posts ALTER COLUMN public_link_eligible TYPE TEXT USING (CASE WHEN public_link_eligible THEN 'DEFAULT' ELSE 'RESTRICTED' END);
ALTER TABLE posts RENAME COLUMN public_link_eligible TO audience_role;
ALTER TABLE posts ALTER COLUMN audience_role SET DEFAULT 'DEFAULT';

CREATE OR REPLACE FUNCTION grant_audience_post(aid UUID, publication UUID, reader UUID, restore BOOLEAN)
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


-- A staged restricted post can opt into journal defaults before publication.
-- Existing published posts and withdrawn grants must not be re-shared implicitly.
CREATE OR REPLACE FUNCTION materialize_post_audiences() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE source traces%ROWTYPE; aid UUID; member UUID; snapshot_defaults BOOLEAN;
BEGIN
    IF NEW.source_trace_id IS NULL OR NEW.audience_source_post_id IS NOT NULL THEN RETURN NEW; END IF;
    SELECT * INTO source FROM traces WHERE id=NEW.source_trace_id;
    IF source.trace_type <> 'USER_TRACE' THEN RETURN NEW; END IF;
    IF NEW.audience_role<>'DEFAULT' THEN
        DELETE FROM post_audiences WHERE post_id=NEW.id AND is_default;
    END IF;
    snapshot_defaults := TG_OP='INSERT';
    IF TG_OP='UPDATE' THEN
        snapshot_defaults := OLD.status='DRAFT' AND OLD.audience_role='RESTRICTED'
            AND NEW.audience_role='DEFAULT';
    END IF;
    IF snapshot_defaults AND NEW.user_id=source.user_id AND NEW.audience_role='DEFAULT'
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

CREATE OR REPLACE FUNCTION archive_trace_publications() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' OR NEW.status <> 'FINALIZED' OR NEW.is_encrypted THEN
        UPDATE posts SET status = 'ARCHIVED', updated_at = NOW()
        WHERE source_trace_id = OLD.id AND status <> 'ARCHIVED';
        UPDATE posts SET status = 'ARCHIVED', updated_at = NOW()
        WHERE source_trace_id IN (SELECT id FROM traces WHERE parent_trace_id = OLD.id)
          AND status <> 'ARCHIVED';
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$;
