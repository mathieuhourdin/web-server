CREATE TABLE publication_channels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('BLOG')),
    enabled BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(id, user_id)
);
CREATE INDEX publication_channels_owner_idx ON publication_channels(user_id);
CREATE TABLE blogs (
    channel_id UUID PRIMARY KEY REFERENCES publication_channels(id) ON DELETE CASCADE,
    slug TEXT NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$' AND length(slug) BETWEEN 1 AND 80),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 160),
    username TEXT NOT NULL CHECK (length(trim(username)) BETWEEN 1 AND 120),
    description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 4000),
    image_asset_id UUID REFERENCES assets(id) ON DELETE SET NULL
);
ALTER TABLE posts ADD COLUMN publication_channel_id UUID;
ALTER TABLE posts ADD CONSTRAINT posts_channel_owner_fk FOREIGN KEY(publication_channel_id,user_id)
    REFERENCES publication_channels(id,user_id) ON DELETE CASCADE;
DROP INDEX idx_posts_trace_publisher;
DROP INDEX idx_posts_unique_source_album_id;
CREATE UNIQUE INDEX idx_posts_trace_publisher ON posts(source_trace_id,user_id)
    WHERE source_trace_id IS NOT NULL AND publication_channel_id IS NULL;
CREATE UNIQUE INDEX idx_posts_unique_source_album_id ON posts(source_album_id)
    WHERE source_album_id IS NOT NULL AND publication_channel_id IS NULL;
CREATE UNIQUE INDEX posts_channel_trace_idx ON posts(publication_channel_id,source_trace_id)
    WHERE publication_channel_id IS NOT NULL AND source_trace_id IS NOT NULL;
CREATE UNIQUE INDEX posts_channel_album_idx ON posts(publication_channel_id,source_album_id)
    WHERE publication_channel_id IS NOT NULL AND source_album_id IS NOT NULL;
CREATE INDEX posts_channel_listing_idx ON posts(publication_channel_id,publishing_date DESC,id DESC)
    WHERE publication_channel_id IS NOT NULL AND status='PUBLISHED';

-- Updatable compatibility boundary: existing social CRUD cannot see or mutate
-- channel publications. The Rust schema maps its legacy `posts` module here.
CREATE VIEW social_posts AS SELECT * FROM posts WHERE publication_channel_id IS NULL
    WITH CASCADED CHECK OPTION;

-- Keep existing publication authorization and inherited-complement lookups
-- social-only too. Preserve the rest of these established functions verbatim.
DO $$ DECLARE f RECORD; BEGIN
    FOR f IN SELECT p.oid FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
        WHERE n.nspname=current_schema() AND p.proname IN
        ('trace_publication_eligible','trace_publication_readable',
         'validate_trace_publication','sync_complement_grants')
    LOOP
        EXECUTE replace(replace(pg_get_functiondef(f.oid),
            'FROM posts ', 'FROM social_posts '), 'JOIN posts ', 'JOIN social_posts ');
    END LOOP;
END $$;

CREATE FUNCTION validate_channel_publication() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='UPDATE' AND (NEW.publication_channel_id IS DISTINCT FROM OLD.publication_channel_id
        OR (NEW.publication_channel_id IS NOT NULL AND NEW.user_id IS DISTINCT FROM OLD.user_id)) THEN
        RAISE EXCEPTION 'Publication destination and publisher are immutable' USING ERRCODE='23514';
    END IF;
    IF NEW.publication_channel_id IS NULL THEN RETURN NEW; END IF;
    IF EXISTS(SELECT 1 FROM automation_demo_users WHERE user_id=NEW.user_id) THEN
        RAISE EXCEPTION 'Demo accounts cannot publish publicly' USING ERRCODE='23514';
    END IF;
    IF NEW.audience_source_post_id IS NOT NULL OR NEW.source_document_id IS NOT NULL
        OR NEW.notification_mode <> 'NONE' THEN
        RAISE EXCEPTION 'Channel posts cannot inherit audiences, publish documents or send social notifications' USING ERRCODE='23514';
    END IF;
    IF NEW.status='PUBLISHED' THEN
        IF NOT EXISTS(SELECT 1 FROM traces WHERE id=NEW.source_trace_id AND user_id=NEW.user_id
                AND status='FINALIZED' AND NOT is_encrypted AND trace_type='USER_TRACE')
            AND NOT EXISTS(SELECT 1 FROM albums WHERE id=NEW.source_album_id AND owner_user_id=NEW.user_id
                AND completion_status <> 'ARCHIVED') THEN
            RAISE EXCEPTION 'Channel source must be an owned publishable trace or album' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER a_validate_channel_publication BEFORE INSERT OR UPDATE ON posts
    FOR EACH ROW EXECUTE FUNCTION validate_channel_publication();

CREATE FUNCTION forbid_channel_grants() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS(SELECT 1 FROM posts WHERE id=NEW.post_id AND publication_channel_id IS NOT NULL) THEN
        RAISE EXCEPTION 'Channel posts do not have social grants or audiences' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER forbid_channel_grants BEFORE INSERT OR UPDATE ON post_grants
    FOR EACH ROW EXECUTE FUNCTION forbid_channel_grants();
CREATE TRIGGER forbid_channel_audiences BEFORE INSERT OR UPDATE ON post_audiences
    FOR EACH ROW EXECUTE FUNCTION forbid_channel_grants();
