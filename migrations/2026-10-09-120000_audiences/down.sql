DROP TRIGGER preserve_active_grant_origin ON post_grants;
DROP FUNCTION preserve_active_grant_origin();
DROP TRIGGER mirror_audience_member_policy ON audience_members;
DROP TRIGGER mirror_journal_policy_audience ON journal_sharing_policies;
DROP FUNCTION mirror_audience_member_policy();
DROP FUNCTION mirror_journal_policy_audience();
DROP TRIGGER validate_post_audience ON post_audiences;
DROP TRIGGER validate_journal_default_audience ON journal_default_audiences;
DROP FUNCTION validate_audience_association();
DROP TRIGGER materialize_post_audiences ON posts;
DROP FUNCTION materialize_post_audiences();
DROP FUNCTION grant_audience_post(UUID,UUID,UUID,BOOLEAN);
CREATE OR REPLACE FUNCTION copy_complement_grants() RETURNS TRIGGER LANGUAGE plpgsql AS $$
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

CREATE OR REPLACE FUNCTION sync_complement_grants() RETURNS TRIGGER LANGUAGE plpgsql AS $$
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


ALTER TABLE post_grants DROP COLUMN audience_id;
DROP TABLE journal_default_audiences;
DROP TABLE post_audiences;
DROP TABLE audience_members;
DROP TABLE audiences;
