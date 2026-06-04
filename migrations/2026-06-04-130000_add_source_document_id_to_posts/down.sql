DROP INDEX IF EXISTS idx_posts_unique_source_document_id;
DROP INDEX IF EXISTS idx_posts_source_document_id;
ALTER TABLE posts DROP CONSTRAINT IF EXISTS posts_single_structured_source_check;
ALTER TABLE posts DROP COLUMN IF EXISTS source_document_id;
