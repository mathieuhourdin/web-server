ALTER TABLE posts
ADD COLUMN status TEXT NOT NULL DEFAULT 'DRAFT';

UPDATE posts
SET status = CASE
    WHEN publishing_state = 'pbsh' AND maturing_state = 'fnsh' THEN 'PUBLISHED'
    ELSE 'DRAFT'
END;

CREATE INDEX idx_posts_status_created_at
ON posts(status, created_at DESC);
