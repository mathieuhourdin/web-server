-- Deliberately removes channel publications, never social posts.
DELETE FROM posts WHERE publication_channel_id IS NOT NULL;
DO $$ DECLARE f RECORD; BEGIN
    FOR f IN SELECT p.oid FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
        WHERE n.nspname=current_schema() AND p.proname IN
        ('trace_publication_eligible','trace_publication_readable',
         'validate_trace_publication','sync_complement_grants')
    LOOP
        EXECUTE replace(pg_get_functiondef(f.oid),'social_posts','posts');
    END LOOP;
END $$;
DROP TRIGGER forbid_channel_grants ON post_grants;
DROP TRIGGER forbid_channel_audiences ON post_audiences;
DROP FUNCTION forbid_channel_grants();
DROP TRIGGER a_validate_channel_publication ON posts;
DROP FUNCTION validate_channel_publication();
DROP VIEW social_posts;
DROP INDEX idx_posts_trace_publisher;
DROP INDEX idx_posts_unique_source_album_id;
DROP INDEX posts_channel_trace_idx;
DROP INDEX posts_channel_album_idx;
DROP INDEX posts_channel_listing_idx;
ALTER TABLE posts DROP COLUMN publication_channel_id;
CREATE UNIQUE INDEX idx_posts_trace_publisher ON posts(source_trace_id,user_id) WHERE source_trace_id IS NOT NULL;
CREATE UNIQUE INDEX idx_posts_unique_source_album_id ON posts(source_album_id) WHERE source_album_id IS NOT NULL;
DROP TABLE blogs;
DROP TABLE publication_channels;
