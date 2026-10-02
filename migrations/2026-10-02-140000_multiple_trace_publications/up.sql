-- Linked journal placements are intentionally retired (no production data).
DELETE FROM traces WHERE trace_type = 'LINKED_TRACE';
ALTER TABLE traces ADD CONSTRAINT traces_no_linked_trace CHECK (trace_type <> 'LINKED_TRACE');
ALTER TABLE traces DROP CONSTRAINT traces_relationship_type_check;
ALTER TABLE traces DROP COLUMN linked_source_trace_id;
ALTER TABLE traces ADD CONSTRAINT traces_relationship_type_check CHECK (
    (trace_type = 'TRACE_COMPLEMENT' AND parent_trace_id IS NOT NULL
        AND complement_audience_mode IS NOT NULL AND journal_id IS NULL)
    OR (trace_type <> 'TRACE_COMPLEMENT' AND parent_trace_id IS NULL
        AND complement_audience_mode IS NULL AND journal_id IS NOT NULL)
);

DROP INDEX idx_posts_unique_source_trace_id;
CREATE UNIQUE INDEX idx_posts_trace_publisher ON posts(source_trace_id, user_id)
    WHERE source_trace_id IS NOT NULL;
ALTER TABLE posts ADD COLUMN audience_source_post_id UUID REFERENCES posts(id) ON DELETE CASCADE;
CREATE INDEX posts_audience_source_idx ON posts(audience_source_post_id)
    WHERE audience_source_post_id IS NOT NULL;
ALTER TABLE post_grants ADD COLUMN source_grant_id UUID REFERENCES post_grants(id) ON DELETE CASCADE;
CREATE UNIQUE INDEX post_grants_inherited_source_idx ON post_grants(post_id, source_grant_id)
    WHERE source_grant_id IS NOT NULL;

-- Used by both single-post checks and bulk candidate selection. No ACL recursion.
CREATE FUNCTION trace_publication_eligible(publication UUID, viewer UUID) RETURNS BOOLEAN
LANGUAGE sql STABLE AS $$
    SELECT EXISTS (
        SELECT 1 FROM posts p
        LEFT JOIN traces t ON t.id = p.source_trace_id
        LEFT JOIN traces parent ON parent.id = t.parent_trace_id
        LEFT JOIN posts audience ON audience.id = p.audience_source_post_id
        WHERE p.id = publication AND p.status = 'PUBLISHED'
          AND (t.id IS NULL OR (
            t.status = 'FINALIZED' AND NOT t.is_encrypted
            AND (t.trace_type <> 'TRACE_COMPLEMENT' OR (
                parent.status = 'FINALIZED' AND NOT parent.is_encrypted
                AND p.user_id = parent.user_id
            ))
            AND (p.user_id = t.user_id OR t.trace_type = 'TRACE_COMPLEMENT' OR EXISTS (
                SELECT 1 FROM trace_mentions m WHERE m.trace_id = t.id
                  AND m.mentioned_user_id = p.user_id AND m.removed_at IS NULL AND m.allows_reshare
            ))
          ))
          AND (p.audience_source_post_id IS NULL OR (
            audience.status = 'PUBLISHED' AND audience.source_trace_id = parent.id
            AND audience.user_id = parent.user_id AND audience.audience_source_post_id IS NULL
          ))
          AND NOT EXISTS (
            SELECT 1 FROM user_blocks b
            WHERE (b.blocker_user_id = viewer AND b.blocked_user_id IN (p.user_id, t.user_id, parent.user_id))
               OR (b.blocked_user_id = viewer AND b.blocker_user_id IN (p.user_id, t.user_id, parent.user_id))
               OR (b.blocker_user_id = p.user_id AND b.blocked_user_id = t.user_id)
               OR (b.blocked_user_id = p.user_id AND b.blocker_user_id = t.user_id)
          )
    );
$$;

CREATE FUNCTION trace_publication_readable(publication UUID, viewer UUID) RETURNS BOOLEAN
LANGUAGE sql STABLE AS $$
    SELECT trace_publication_eligible(publication, viewer) AND EXISTS (
        SELECT 1 FROM posts p WHERE p.id = publication AND (
            p.user_id = viewer OR EXISTS (
                SELECT 1 FROM post_grants g WHERE g.post_id = p.id AND g.status = 'ACTIVE'
                AND (g.grantee_user_id = viewer OR (g.grantee_scope = 'ALL_PLATFORM_USERS'
                    AND EXISTS (SELECT 1 FROM users u WHERE u.id = viewer
                        AND u.is_platform_user AND u.principal_type = 'HUMAN')))
            )
        )
    );
$$;

-- Validate publication authority under the source/mention locks, including
-- concurrent source archival and mention revocation. Drafts remain private.
CREATE FUNCTION validate_trace_publication() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE source traces%ROWTYPE; parent traces%ROWTYPE; audience posts%ROWTYPE;
BEGIN
    IF NEW.source_trace_id IS NULL THEN RETURN NEW; END IF;
    SELECT * INTO source FROM traces WHERE id = NEW.source_trace_id FOR SHARE;
    IF source.trace_type = 'TRACE_COMPLEMENT' THEN
        SELECT * INTO parent FROM traces WHERE id = source.parent_trace_id FOR SHARE;
        IF source.complement_audience_mode = 'PARENT' THEN
            SELECT * INTO audience FROM posts
            WHERE source_trace_id = parent.id AND user_id = parent.user_id FOR UPDATE;
            IF audience.id IS NULL THEN
                RAISE EXCEPTION 'Parent publication required for inherited complement' USING ERRCODE = '23514';
            END IF;
            NEW.audience_source_post_id := audience.id;
        ELSIF NEW.audience_source_post_id IS NOT NULL THEN
            RAISE EXCEPTION 'Independent complement cannot inherit grants' USING ERRCODE = '23514';
        END IF;
    ELSIF NEW.audience_source_post_id IS NOT NULL THEN
        RAISE EXCEPTION 'Only complements can inherit grants' USING ERRCODE = '23514';
    END IF;
    IF NEW.status <> 'PUBLISHED' THEN RETURN NEW; END IF;
    IF source.status <> 'FINALIZED' OR source.is_encrypted THEN
        RAISE EXCEPTION 'Source does not permit publication' USING ERRCODE = '23514';
    END IF;
    IF source.trace_type = 'TRACE_COMPLEMENT' THEN
        IF NEW.user_id <> parent.user_id OR parent.status <> 'FINALIZED' OR parent.is_encrypted THEN
            RAISE EXCEPTION 'Only the parent author can publish a complement' USING ERRCODE = '23514';
        END IF;
    ELSIF NEW.user_id <> source.user_id THEN
        PERFORM 1 FROM trace_mentions WHERE trace_id = source.id
          AND mentioned_user_id = NEW.user_id AND removed_at IS NULL AND allows_reshare FOR SHARE;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'Mention does not permit publication' USING ERRCODE = '23514';
        END IF;
    END IF;
    IF EXISTS (SELECT 1 FROM user_blocks WHERE
        (blocker_user_id = source.user_id AND blocked_user_id = NEW.user_id) OR
        (blocked_user_id = source.user_id AND blocker_user_id = NEW.user_id)) THEN
        RAISE EXCEPTION 'Publication blocked' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION copy_complement_grants() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.audience_source_post_id IS NOT NULL THEN
        INSERT INTO post_grants(id, post_id, owner_user_id, grantee_user_id,
            grantee_scope, access_level, status, source_grant_id)
        SELECT uuid_generate_v4(), NEW.id, NEW.user_id, g.grantee_user_id,
            g.grantee_scope, g.access_level, g.status, g.id
        FROM post_grants g WHERE g.post_id = NEW.audience_source_post_id
        ON CONFLICT (post_id, source_grant_id) WHERE source_grant_id IS NOT NULL
        DO UPDATE SET status = EXCLUDED.status, access_level = EXCLUDED.access_level, updated_at = NOW();
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION sync_complement_grants() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE publication UUID;
BEGIN
    publication := CASE WHEN TG_OP = 'DELETE' THEN OLD.post_id ELSE NEW.post_id END;
    -- Serialize parent grant mutations with creation of inherited posts.
    PERFORM 1 FROM posts WHERE id = publication FOR UPDATE;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF; -- inherited rows cascade
    IF NEW.source_grant_id IS NOT NULL THEN RETURN NEW; END IF;
    INSERT INTO post_grants(id, post_id, owner_user_id, grantee_user_id,
        grantee_scope, access_level, status, source_grant_id)
    SELECT uuid_generate_v4(), p.id, p.user_id, NEW.grantee_user_id,
        NEW.grantee_scope, NEW.access_level, NEW.status, NEW.id
    FROM posts p WHERE p.audience_source_post_id = publication
    ON CONFLICT (post_id, source_grant_id) WHERE source_grant_id IS NOT NULL
    DO UPDATE SET grantee_user_id = EXCLUDED.grantee_user_id, grantee_scope = EXCLUDED.grantee_scope,
        access_level = EXCLUDED.access_level, status = EXCLUDED.status, updated_at = NOW();
    RETURN NEW;
END;
$$;

-- Existing contributors' publications now require the parent owner's approval.
UPDATE posts p SET status = 'DRAFT', publishing_date = NULL
FROM traces t, traces parent
WHERE p.source_trace_id = t.id AND t.parent_trace_id = parent.id
  AND t.trace_type = 'TRACE_COMPLEMENT' AND p.user_id <> parent.user_id;
UPDATE posts p SET audience_source_post_id = owner_post.id
FROM traces t, traces parent, posts owner_post
WHERE p.source_trace_id = t.id AND t.parent_trace_id = parent.id
  AND t.complement_audience_mode = 'PARENT'
  AND owner_post.source_trace_id = parent.id AND owner_post.user_id = parent.user_id;
INSERT INTO post_grants(id, post_id, owner_user_id, grantee_user_id,
    grantee_scope, access_level, status, source_grant_id)
SELECT uuid_generate_v4(), p.id, p.user_id, g.grantee_user_id,
    g.grantee_scope, g.access_level, g.status, g.id
FROM posts p JOIN post_grants g ON g.post_id = p.audience_source_post_id;

CREATE TRIGGER validate_trace_publication BEFORE INSERT OR UPDATE ON posts
FOR EACH ROW EXECUTE FUNCTION validate_trace_publication();
CREATE TRIGGER copy_complement_grants AFTER INSERT ON posts
FOR EACH ROW EXECUTE FUNCTION copy_complement_grants();
CREATE TRIGGER sync_complement_grants AFTER INSERT OR UPDATE OR DELETE ON post_grants
FOR EACH ROW EXECUTE FUNCTION sync_complement_grants();

-- Revocation does not silently re-publish when permission is later restored.
CREATE FUNCTION revoke_mention_publications() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.removed_at IS NOT NULL OR NOT NEW.allows_reshare THEN
        UPDATE posts SET status = 'ARCHIVED', updated_at = NOW()
        WHERE source_trace_id = NEW.trace_id AND user_id = NEW.mentioned_user_id;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER revoke_mention_publications AFTER UPDATE ON trace_mentions
FOR EACH ROW EXECUTE FUNCTION revoke_mention_publications();

CREATE FUNCTION archive_trace_publications() RETURNS TRIGGER LANGUAGE plpgsql AS $$
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
CREATE TRIGGER archive_trace_publications BEFORE UPDATE OF status, is_encrypted OR DELETE ON traces
FOR EACH ROW EXECUTE FUNCTION archive_trace_publications();
