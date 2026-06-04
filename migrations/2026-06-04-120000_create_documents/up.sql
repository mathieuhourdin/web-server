CREATE TABLE documents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    document_role TEXT NOT NULL CHECK (document_role IN ('CREATION', 'REFERENCE')),
    document_type TEXT CHECK (
        document_type IS NULL OR document_type IN (
            'IDEA',
            'RESEARCH_ARTICLE',
            'BOOK',
            'COURSE',
            'QUESTION',
            'OPINION_ARTICLE',
            'PROBLEM',
            'PODCAST',
            'NEWS_ARTICLE',
            'READING_NOTE',
            'RESOURCE_LIST'
        )
    ),
    content_source TEXT NOT NULL CHECK (content_source IN ('DB_CONTENT', 'INTERNAL_ASSET', 'EXTERNAL_URL', 'REFERENCE_ONLY')),
    title TEXT NOT NULL DEFAULT '',
    subtitle TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    author_name TEXT,
    content TEXT,
    asset_id UUID REFERENCES assets(id),
    external_content_url TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT documents_content_source_payload_check CHECK (
        (
            content_source = 'DB_CONTENT'
            AND content IS NOT NULL
            AND asset_id IS NULL
            AND external_content_url IS NULL
        )
        OR (
            content_source = 'INTERNAL_ASSET'
            AND content IS NULL
            AND asset_id IS NOT NULL
            AND external_content_url IS NULL
        )
        OR (
            content_source = 'EXTERNAL_URL'
            AND content IS NULL
            AND asset_id IS NULL
            AND external_content_url IS NOT NULL
        )
        OR (
            content_source = 'REFERENCE_ONLY'
            AND content IS NULL
            AND asset_id IS NULL
            AND external_content_url IS NULL
        )
    )
);

CREATE INDEX documents_owner_user_id_idx ON documents(owner_user_id);
CREATE INDEX documents_asset_id_idx ON documents(asset_id);

SELECT diesel_manage_updated_at('documents');
