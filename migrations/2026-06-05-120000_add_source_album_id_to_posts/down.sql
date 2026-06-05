DROP INDEX IF EXISTS idx_posts_unique_source_album_id;
DROP INDEX IF EXISTS idx_posts_source_album_id;

ALTER TABLE posts
DROP CONSTRAINT IF EXISTS posts_single_structured_source_check;

ALTER TABLE posts
ADD CONSTRAINT posts_single_structured_source_check
CHECK (
    source_trace_id IS NULL
    OR source_document_id IS NULL
);

ALTER TABLE posts DROP COLUMN IF EXISTS source_album_id;
