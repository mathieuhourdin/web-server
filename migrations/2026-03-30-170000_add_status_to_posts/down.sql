DROP INDEX IF EXISTS idx_posts_status_created_at;

ALTER TABLE posts
DROP COLUMN IF EXISTS status;
