ALTER TABLE posts
ADD COLUMN source_document_id UUID NULL REFERENCES documents(id) ON DELETE SET NULL;

ALTER TABLE posts
ADD CONSTRAINT posts_single_structured_source_check
CHECK (
    source_trace_id IS NULL
    OR source_document_id IS NULL
);

CREATE INDEX idx_posts_source_document_id ON posts(source_document_id);

CREATE UNIQUE INDEX idx_posts_unique_source_document_id
ON posts (source_document_id)
WHERE source_document_id IS NOT NULL;
