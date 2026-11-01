-- Keep one physical publication table. Social access is an explicit condition,
-- not a second relation or a transparent Rust-to-SQL name mapping.
CREATE OR REPLACE FUNCTION trace_publication_eligible(publication UUID, viewer UUID) RETURNS BOOLEAN
LANGUAGE sql STABLE AS $$
    SELECT EXISTS (
        SELECT 1 FROM posts p
        LEFT JOIN traces t ON t.id = p.source_trace_id
        LEFT JOIN traces parent ON parent.id = t.parent_trace_id
        LEFT JOIN posts audience ON audience.id = p.audience_source_post_id
        WHERE p.id = publication AND p.publication_channel_id IS NULL AND p.status = 'PUBLISHED'
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
            audience.publication_channel_id IS NULL AND audience.status = 'PUBLISHED'
            AND audience.source_trace_id = parent.id
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

CREATE OR REPLACE FUNCTION trace_publication_readable(publication UUID,viewer UUID) RETURNS BOOLEAN
LANGUAGE SQL STABLE AS $$
    SELECT trace_publication_eligible(publication,viewer) AND EXISTS (
        SELECT 1 FROM posts p WHERE p.id=publication AND p.publication_channel_id IS NULL
        AND automation_same_sandbox(p.user_id,viewer)
        AND (p.user_id=viewer OR EXISTS (
            SELECT 1 FROM post_grants g WHERE g.post_id=p.id AND g.status='ACTIVE'
            AND (g.grantee_user_id=viewer OR (g.grantee_scope='ALL_PLATFORM_USERS'
                AND EXISTS(SELECT 1 FROM users u WHERE u.id=viewer AND u.is_platform_user AND u.principal_type='HUMAN')))
        ))
    )
$$;

CREATE OR REPLACE FUNCTION validate_trace_publication() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE source traces%ROWTYPE; parent traces%ROWTYPE; audience posts%ROWTYPE;
BEGIN
    IF NEW.source_trace_id IS NULL THEN RETURN NEW; END IF;
    SELECT * INTO source FROM traces WHERE id = NEW.source_trace_id FOR SHARE;
    IF source.trace_type = 'TRACE_COMPLEMENT' THEN
        SELECT * INTO parent FROM traces WHERE id = source.parent_trace_id FOR SHARE;
        IF source.complement_audience_mode = 'PARENT' THEN
            SELECT * INTO audience FROM posts WHERE publication_channel_id IS NULL
                AND source_trace_id = parent.id AND user_id = parent.user_id FOR UPDATE;
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

CREATE OR REPLACE FUNCTION sync_complement_grants() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE publication UUID;
BEGIN
    publication := CASE WHEN TG_OP='DELETE' THEN OLD.post_id ELSE NEW.post_id END;
    PERFORM 1 FROM posts WHERE publication_channel_id IS NULL AND id=publication FOR UPDATE;
    IF TG_OP='DELETE' THEN RETURN OLD; END IF;
    IF NEW.source_grant_id IS NOT NULL THEN RETURN NEW; END IF;
    INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,grantee_scope,access_level,status,source_grant_id,audience_id)
    SELECT uuid_generate_v4(),p.id,p.user_id,NEW.grantee_user_id,NEW.grantee_scope,NEW.access_level,NEW.status,NEW.id,NEW.audience_id
    FROM posts p WHERE p.publication_channel_id IS NULL AND p.audience_source_post_id=publication
    ON CONFLICT(post_id,source_grant_id) WHERE source_grant_id IS NOT NULL
    DO UPDATE SET grantee_user_id=EXCLUDED.grantee_user_id,grantee_scope=EXCLUDED.grantee_scope,
        access_level=EXCLUDED.access_level,status=EXCLUDED.status,audience_id=EXCLUDED.audience_id,updated_at=NOW();
    RETURN NEW;
END;
$$;

-- No data is copied, deleted, regranted or published by this migration.
DROP VIEW social_posts;
