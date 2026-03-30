DROP TABLE IF EXISTS post_grants;
DROP INDEX IF EXISTS idx_posts_source_trace_id;
ALTER TABLE posts DROP COLUMN IF EXISTS source_trace_id;
