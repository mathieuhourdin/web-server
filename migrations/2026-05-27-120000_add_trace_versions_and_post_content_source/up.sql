CREATE TABLE trace_versions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    version_number INTEGER NULL,
    status TEXT NOT NULL DEFAULT 'DRAFT',
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    content TEXT NOT NULL,
    image_asset_id UUID NULL REFERENCES assets(id) ON DELETE SET NULL,
    interaction_date TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    finalized_at TIMESTAMP NULL,
    CONSTRAINT trace_versions_status_check
        CHECK (status IN ('DRAFT', 'FINALIZED', 'ARCHIVED')),
    CONSTRAINT trace_versions_finalized_number_check
        CHECK (
            (status = 'FINALIZED' AND version_number IS NOT NULL AND finalized_at IS NOT NULL)
            OR status <> 'FINALIZED'
        )
);

ALTER TABLE traces
ADD COLUMN current_version_id UUID NULL REFERENCES trace_versions(id) ON DELETE SET NULL;

ALTER TABLE posts
ADD COLUMN trace_version_id UUID NULL REFERENCES trace_versions(id) ON DELETE SET NULL,
ADD COLUMN content_source TEXT NOT NULL DEFAULT 'TRACE_VERSION',
ADD CONSTRAINT posts_content_source_check
    CHECK (content_source IN ('TRACE_VERSION', 'CUSTOM'));

ALTER TABLE landscape_analyses
ADD COLUMN trace_version_id UUID NULL REFERENCES trace_versions(id) ON DELETE SET NULL;

CREATE UNIQUE INDEX idx_trace_versions_trace_id_version_number_unique
ON trace_versions (trace_id, version_number)
WHERE version_number IS NOT NULL;

CREATE UNIQUE INDEX idx_trace_versions_unique_draft
ON trace_versions (trace_id)
WHERE status = 'DRAFT';

CREATE INDEX idx_trace_versions_trace_id_status
ON trace_versions (trace_id, status);

CREATE INDEX idx_traces_current_version_id
ON traces (current_version_id);

CREATE INDEX idx_posts_trace_version_id
ON posts (trace_version_id);

CREATE INDEX idx_landscape_analyses_trace_version_id
ON landscape_analyses (trace_version_id);

WITH inserted_versions AS (
    INSERT INTO trace_versions (
        id,
        trace_id,
        version_number,
        status,
        title,
        subtitle,
        content,
        image_asset_id,
        interaction_date,
        created_at,
        updated_at,
        finalized_at
    )
    SELECT
        uuid_generate_v4(),
        traces.id,
        CASE WHEN traces.status = 'DRAFT' THEN NULL ELSE 1 END,
        CASE WHEN traces.status = 'DRAFT' THEN 'DRAFT' ELSE 'FINALIZED' END,
        traces.title,
        traces.subtitle,
        traces.content,
        traces.image_asset_id,
        traces.interaction_date,
        traces.created_at,
        traces.updated_at,
        CASE WHEN traces.status = 'DRAFT' THEN NULL ELSE COALESCE(traces.finalized_at, traces.created_at) END
    FROM traces
    WHERE traces.status <> 'ARCHIVED'
    RETURNING id, trace_id
)
UPDATE traces
SET current_version_id = inserted_versions.id
FROM inserted_versions
WHERE traces.id = inserted_versions.trace_id
  AND traces.status <> 'DRAFT';

UPDATE posts
SET trace_version_id = traces.current_version_id
FROM traces
WHERE posts.source_trace_id = traces.id
  AND traces.current_version_id IS NOT NULL;

UPDATE posts
SET content_source = 'CUSTOM'
WHERE source_trace_id IS NULL
   OR trace_version_id IS NULL;

UPDATE landscape_analyses
SET trace_version_id = traces.current_version_id
FROM traces
WHERE landscape_analyses.analyzed_trace_id = traces.id
  AND traces.current_version_id IS NOT NULL;
