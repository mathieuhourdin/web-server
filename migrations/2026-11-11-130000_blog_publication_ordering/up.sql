ALTER TABLE posts ADD COLUMN ordering_date timestamp;
ALTER TABLE posts ADD CONSTRAINT posts_channel_ordering_date_check
    CHECK (ordering_date IS NULL OR publication_channel_id IS NOT NULL);
CREATE INDEX posts_channel_ordering_idx ON posts(publication_channel_id,
    (COALESCE(ordering_date,last_distribution_at,publishing_date)) DESC NULLS LAST,id DESC)
    WHERE publication_channel_id IS NOT NULL;

-- Reordering is metadata-only, including for an unavailable/archived source.
-- Keep source, destination, ownership and publication-authority validation intact.
DROP TRIGGER a_validate_channel_publication ON posts;
CREATE TRIGGER a_validate_channel_publication BEFORE INSERT OR UPDATE OF
    publication_channel_id,user_id,source_trace_id,source_album_id,source_document_id,
    status,notification_mode,audience_source_post_id ON posts
    FOR EACH ROW EXECUTE FUNCTION validate_channel_publication();
