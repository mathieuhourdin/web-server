-- Refuse rollback while multiple publishers remain; never delete publications silently.
DROP TRIGGER archive_trace_publications ON traces;
DROP FUNCTION archive_trace_publications();
DROP TRIGGER revoke_mention_publications ON trace_mentions;
DROP FUNCTION revoke_mention_publications();
DROP TRIGGER sync_complement_grants ON post_grants;
DROP TRIGGER copy_complement_grants ON posts;
DROP TRIGGER validate_trace_publication ON posts;
DROP FUNCTION sync_complement_grants();
DROP FUNCTION copy_complement_grants();
DROP FUNCTION validate_trace_publication();
DROP FUNCTION trace_publication_readable(UUID, UUID);
DROP FUNCTION trace_publication_eligible(UUID, UUID);
DELETE FROM post_grants WHERE source_grant_id IS NOT NULL;
ALTER TABLE post_grants DROP COLUMN source_grant_id;
ALTER TABLE posts DROP COLUMN audience_source_post_id;
DROP INDEX idx_posts_trace_publisher;
CREATE UNIQUE INDEX idx_posts_unique_source_trace_id ON posts(source_trace_id)
    WHERE source_trace_id IS NOT NULL;
ALTER TABLE traces DROP CONSTRAINT traces_no_linked_trace;
ALTER TABLE traces DROP CONSTRAINT traces_relationship_type_check;
ALTER TABLE traces ADD COLUMN linked_source_trace_id UUID REFERENCES traces(id) ON DELETE CASCADE;
CREATE UNIQUE INDEX traces_linked_source_per_user_idx ON traces(user_id, linked_source_trace_id)
    WHERE trace_type = 'LINKED_TRACE';
CREATE INDEX traces_linked_source_idx ON traces(linked_source_trace_id)
    WHERE linked_source_trace_id IS NOT NULL;
ALTER TABLE traces ADD CONSTRAINT traces_relationship_type_check CHECK (
    (trace_type = 'LINKED_TRACE' AND linked_source_trace_id IS NOT NULL AND parent_trace_id IS NULL
        AND complement_audience_mode IS NULL AND journal_id IS NOT NULL)
    OR (trace_type = 'TRACE_COMPLEMENT' AND parent_trace_id IS NOT NULL AND linked_source_trace_id IS NULL
        AND complement_audience_mode IS NOT NULL AND journal_id IS NULL)
    OR (trace_type NOT IN ('LINKED_TRACE', 'TRACE_COMPLEMENT') AND linked_source_trace_id IS NULL
        AND parent_trace_id IS NULL AND complement_audience_mode IS NULL AND journal_id IS NOT NULL)
);
