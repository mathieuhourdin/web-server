CREATE TABLE trace_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    source_version INTEGER NOT NULL,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    content TEXT NOT NULL,
    content_image_asset_id UUID REFERENCES assets(id) ON DELETE SET NULL,
    language_code TEXT,
    finalized_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE INDEX trace_versions_history_idx ON trace_versions(trace_id,created_at DESC,id DESC);
CREATE TABLE post_publications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    post_id UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    published_at TIMESTAMP NOT NULL DEFAULT clock_timestamp(),
    trace_version_id UUID REFERENCES trace_versions(id) ON DELETE SET NULL,
    notification_mode TEXT NOT NULL CHECK(notification_mode IN ('ALL','NONE','SELECTED')),
    notification_user_ids UUID[] NOT NULL DEFAULT '{}',
    idempotency_key UUID NOT NULL DEFAULT gen_random_uuid(),
    dispatch_started_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(post_id,idempotency_key)
);
CREATE INDEX post_publications_history_idx ON post_publications(post_id,published_at DESC,id DESC);
CREATE UNIQUE INDEX outbound_email_publication_once_idx ON outbound_emails(resource_id,recipient_user_id,reason)
    WHERE resource_type='POST_PUBLICATION';
CREATE TABLE post_publication_recipients (
    publication_id UUID NOT NULL REFERENCES post_publications(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    push_status TEXT NOT NULL DEFAULT 'NOT_ATTEMPTED'
        CHECK(push_status IN ('NOT_ATTEMPTED','SENDING','SENT','FAILED','SKIPPED')),
    push_sent_at TIMESTAMP,
    email_id UUID REFERENCES outbound_emails(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY(publication_id,user_id)
);
ALTER TABLE posts ADD COLUMN last_publication_id UUID REFERENCES post_publications(id) ON DELETE SET NULL;
ALTER TABLE posts ADD COLUMN last_publication_at TIMESTAMP;
CREATE INDEX posts_last_publication_idx ON posts(last_publication_at DESC,id DESC) WHERE status='PUBLISHED';

-- Compatibility columns on posts remain preferences for older clients only.
-- Historical recipients are unknown unless a successful delivery was recorded.
INSERT INTO post_publications(post_id,published_at,notification_mode,notification_user_ids,dispatch_started_at)
SELECT id,COALESCE(publishing_date,created_at),notification_mode,notification_user_ids,NOW()
FROM posts WHERE publishing_date IS NOT NULL OR status='PUBLISHED';
UPDATE posts p SET last_publication_id=e.id,last_publication_at=e.published_at
FROM post_publications e WHERE e.post_id=p.id;
INSERT INTO post_publication_recipients(publication_id,user_id,push_status,push_sent_at)
SELECT e.id,d.recipient_user_id,'SENT',d.sent_at FROM publication_push_deliveries d
JOIN post_publications e ON e.post_id=d.post_id JOIN posts p ON p.id=d.post_id
WHERE d.publishing_date IS NOT DISTINCT FROM p.publishing_date;

CREATE OR REPLACE VIEW social_posts AS SELECT * FROM posts WHERE publication_channel_id IS NULL
    WITH CASCADED CHECK OPTION;

CREATE FUNCTION validate_post_publication_event() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE publication posts%ROWTYPE; BEGIN
    SELECT * INTO publication FROM posts WHERE id=NEW.post_id FOR UPDATE;
    IF publication.id IS NULL OR publication.status<>'PUBLISHED' THEN
        RAISE EXCEPTION 'Post must be published before creating a publication event' USING ERRCODE='23514';
    END IF;
    IF publication.publication_channel_id IS NULL THEN
        IF NOT trace_publication_eligible(publication.id,publication.user_id) THEN
            RAISE EXCEPTION 'Publisher is no longer authorized' USING ERRCODE='23514';
        END IF;
    ELSIF NEW.notification_mode<>'NONE' OR cardinality(NEW.notification_user_ids)>0 THEN
        RAISE EXCEPTION 'Channel publications do not send social notifications' USING ERRCODE='23514';
    END IF;
    IF NEW.trace_version_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM trace_versions v WHERE v.id=NEW.trace_version_id AND v.trace_id=publication.source_trace_id
    ) THEN
        RAISE EXCEPTION 'Archived version must belong to the published trace' USING ERRCODE='23514';
    END IF;
    IF (NEW.notification_mode='SELECTED' AND cardinality(NEW.notification_user_ids) NOT BETWEEN 1 AND 500)
        OR (NEW.notification_mode<>'SELECTED' AND cardinality(NEW.notification_user_ids)>0) THEN
        RAISE EXCEPTION 'Invalid publication notification selection' USING ERRCODE='23514';
    END IF;
    IF NEW.notification_mode='SELECTED' AND EXISTS(
        SELECT 1 FROM unnest(NEW.notification_user_ids) reader WHERE reader=publication.user_id
            OR NOT trace_publication_readable(publication.id,reader)
            OR NOT EXISTS(SELECT 1 FROM users u WHERE u.id=reader AND u.principal_type='HUMAN')
    ) THEN
        RAISE EXCEPTION 'Recipients must already have access' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER validate_post_publication_event BEFORE INSERT ON post_publications
    FOR EACH ROW EXECUTE FUNCTION validate_post_publication_event();

CREATE FUNCTION materialize_post_publication_event() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO post_publication_recipients(publication_id,user_id)
    SELECT NEW.id,u.id FROM users u JOIN posts p ON p.id=NEW.post_id
    WHERE NEW.notification_mode<>'NONE' AND p.publication_channel_id IS NULL
        AND u.id<>p.user_id AND u.principal_type='HUMAN'
        AND (NEW.notification_mode='ALL' OR u.id=ANY(NEW.notification_user_ids))
        AND (EXISTS(SELECT 1 FROM post_grants g WHERE g.post_id=p.id AND g.status='ACTIVE' AND g.grantee_user_id=u.id)
            OR EXISTS(SELECT 1 FROM post_grants g WHERE g.post_id=p.id AND g.status='ACTIVE' AND g.grantee_scope='ALL_PLATFORM_USERS' AND u.is_platform_user))
        AND trace_publication_readable(p.id,u.id);
    UPDATE posts SET last_publication_id=NEW.id,last_publication_at=NEW.published_at
        WHERE id=NEW.post_id;
    RETURN NEW;
END $$;
CREATE TRIGGER materialize_post_publication_event AFTER INSERT ON post_publications
    FOR EACH ROW EXECUTE FUNCTION materialize_post_publication_event();

-- The z-prefix runs after inherited complement grants have been copied.
CREATE FUNCTION record_post_publish_transition() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.status='PUBLISHED' AND (TG_OP='INSERT' OR OLD.status<>'PUBLISHED') THEN
        INSERT INTO post_publications(post_id,published_at,notification_mode,notification_user_ids)
        VALUES(NEW.id,CASE WHEN NEW.last_publication_id IS NULL THEN COALESCE(NEW.publishing_date,clock_timestamp()) ELSE clock_timestamp() END,
            NEW.notification_mode,NEW.notification_user_ids);
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER zz_record_post_publish_transition AFTER INSERT OR UPDATE OF status ON posts
    FOR EACH ROW EXECUTE FUNCTION record_post_publish_transition();

CREATE FUNCTION immutable_trace_version() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    -- Allow FK-driven clearing when an image is permanently deleted, not edits.
    IF (to_jsonb(NEW)-'content_image_asset_id') IS DISTINCT FROM (to_jsonb(OLD)-'content_image_asset_id')
        OR (NEW.content_image_asset_id IS DISTINCT FROM OLD.content_image_asset_id AND NEW.content_image_asset_id IS NOT NULL) THEN
        RAISE EXCEPTION 'Archived trace versions cannot be edited' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER immutable_trace_version BEFORE UPDATE ON trace_versions FOR EACH ROW EXECUTE FUNCTION immutable_trace_version();
CREATE FUNCTION immutable_post_publication() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF (to_jsonb(NEW)-'dispatch_started_at'-'trace_version_id') IS DISTINCT FROM (to_jsonb(OLD)-'dispatch_started_at'-'trace_version_id')
        OR (NEW.trace_version_id IS DISTINCT FROM OLD.trace_version_id AND NEW.trace_version_id IS NOT NULL) THEN
        RAISE EXCEPTION 'Publication history cannot be edited' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER immutable_post_publication BEFORE UPDATE ON post_publications FOR EACH ROW EXECUTE FUNCTION immutable_post_publication();
CREATE FUNCTION delete_publication_emails() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    DELETE FROM outbound_emails WHERE resource_type='POST_PUBLICATION' AND resource_id=OLD.id;
    RETURN OLD;
END $$;
CREATE TRIGGER delete_publication_emails AFTER DELETE ON post_publications FOR EACH ROW EXECUTE FUNCTION delete_publication_emails();
