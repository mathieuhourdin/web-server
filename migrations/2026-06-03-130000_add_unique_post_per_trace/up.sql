CREATE UNIQUE INDEX idx_posts_unique_source_trace_id
ON posts (source_trace_id)
WHERE source_trace_id IS NOT NULL;
