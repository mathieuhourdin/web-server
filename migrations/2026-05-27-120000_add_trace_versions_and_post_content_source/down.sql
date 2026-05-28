ALTER TABLE landscape_analyses
DROP COLUMN IF EXISTS trace_version_id;

ALTER TABLE posts
DROP CONSTRAINT IF EXISTS posts_content_source_check,
DROP COLUMN IF EXISTS content_source,
DROP COLUMN IF EXISTS trace_version_id;

ALTER TABLE traces
DROP COLUMN IF EXISTS current_version_id;

DROP TABLE IF EXISTS trace_versions;
