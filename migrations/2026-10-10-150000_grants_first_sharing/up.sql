-- Publication no longer selects or expands an audience.
DROP TRIGGER materialize_post_audiences ON posts;
DROP FUNCTION materialize_post_audiences();

-- Preserve public-link exclusions independently of sharing recipients.
ALTER TABLE posts RENAME COLUMN audience_role TO public_link_eligible;
ALTER TABLE posts ALTER COLUMN public_link_eligible DROP DEFAULT;
ALTER TABLE posts ALTER COLUMN public_link_eligible TYPE BOOLEAN USING (public_link_eligible='DEFAULT');
-- New posts use trace sensitivity for public-link eligibility. FALSE is legacy
-- exclusion metadata only, never an audience-selection mode.
ALTER TABLE posts ALTER COLUMN public_link_eligible SET DEFAULT TRUE;

-- Editing a draft trace must not archive its prepared draft post.
CREATE OR REPLACE FUNCTION archive_trace_publications() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        UPDATE posts SET status='ARCHIVED',updated_at=NOW()
        WHERE (source_trace_id=OLD.id OR source_trace_id IN (SELECT id FROM traces WHERE parent_trace_id=OLD.id))
            AND status<>'ARCHIVED';
        RETURN OLD;
    END IF;
    IF NEW.status<>'FINALIZED' OR NEW.is_encrypted THEN
        UPDATE posts SET status='ARCHIVED',updated_at=NOW()
        WHERE (source_trace_id=OLD.id OR source_trace_id IN (SELECT id FROM traces WHERE parent_trace_id=OLD.id))
            AND status<>'ARCHIVED'
            AND (status='PUBLISHED' OR NEW.status='ARCHIVED' OR NEW.is_encrypted);
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION grant_audience_post(aid UUID, publication UUID, reader UUID, restore BOOLEAN)
RETURNS BOOLEAN LANGUAGE plpgsql AS $$
DECLARE owner UUID; changed UUID;
BEGIN
    SELECT owner_user_id INTO owner FROM audiences WHERE id=aid AND archived_at IS NULL FOR UPDATE;
    IF owner IS NULL OR NOT EXISTS (SELECT 1 FROM post_audiences WHERE audience_id=aid AND post_id=publication)
        OR NOT EXISTS (SELECT 1 FROM audience_members WHERE audience_id=aid AND user_id=reader
            AND (status='ACTIVE' OR (restore AND status='SUGGESTED')))
        OR NOT EXISTS (SELECT 1 FROM posts p JOIN traces t ON t.id=p.source_trace_id
            WHERE p.id=publication AND p.user_id=owner AND t.trace_type='USER_TRACE') THEN RETURN FALSE; END IF;
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
    INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,access_level,status,audience_id)
    VALUES(uuid_generate_v4(),publication,owner,reader,'READ','ACTIVE',aid)
    ON CONFLICT(post_id,grantee_user_id) WHERE grantee_user_id IS NOT NULL
    DO UPDATE SET status='ACTIVE', updated_at=NOW()
    WHERE post_grants.status <> 'ACTIVE' AND restore AND post_grants.source_grant_id IS NULL
    RETURNING id INTO changed;
    RETURN changed IS NOT NULL;
END;
$$;
