ALTER TABLE posts
ADD COLUMN source_album_id UUID NULL REFERENCES albums(id) ON DELETE SET NULL;

ALTER TABLE posts
DROP CONSTRAINT IF EXISTS posts_single_structured_source_check;

ALTER TABLE posts
ADD CONSTRAINT posts_single_structured_source_check
CHECK (
    num_nonnulls(source_trace_id, source_document_id, source_album_id) <= 1
);

CREATE INDEX idx_posts_source_album_id ON posts(source_album_id);

CREATE UNIQUE INDEX idx_posts_unique_source_album_id
ON posts (source_album_id)
WHERE source_album_id IS NOT NULL;
