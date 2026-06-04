ALTER TABLE documents
ADD COLUMN status TEXT NOT NULL DEFAULT 'ACTIVE';

ALTER TABLE documents
ADD CONSTRAINT documents_status_check CHECK (
    status IN ('ACTIVE', 'ARCHIVED')
);
