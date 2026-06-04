ALTER TABLE documents
ADD COLUMN content_format TEXT;

UPDATE documents
SET content_format = 'INTERNAL'
WHERE content_source = 'DB_CONTENT';

ALTER TABLE documents
ADD CONSTRAINT documents_content_format_check CHECK (
    (
        content_source = 'DB_CONTENT'
        AND content_format IN ('PLAIN_TEXT', 'MARKDOWN', 'INTERNAL')
    )
    OR (
        content_source <> 'DB_CONTENT'
        AND content_format IS NULL
    )
);
