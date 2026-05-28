ALTER TABLE trace_versions
ADD COLUMN IF NOT EXISTS status TEXT,
ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP,
ADD COLUMN IF NOT EXISTS finalized_at TIMESTAMP NULL;

UPDATE trace_versions
SET status = CASE
    WHEN version_number IS NULL THEN 'DRAFT'
    ELSE 'FINALIZED'
END
WHERE status IS NULL;

UPDATE trace_versions
SET updated_at = created_at
WHERE updated_at IS NULL;

UPDATE trace_versions
SET finalized_at = COALESCE(finalized_at, created_at)
WHERE status = 'FINALIZED'
  AND finalized_at IS NULL;

ALTER TABLE trace_versions
ALTER COLUMN version_number DROP NOT NULL,
ALTER COLUMN status SET DEFAULT 'DRAFT',
ALTER COLUMN status SET NOT NULL,
ALTER COLUMN updated_at SET DEFAULT NOW(),
ALTER COLUMN updated_at SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'trace_versions_status_check'
          AND conrelid = 'trace_versions'::regclass
    ) THEN
        ALTER TABLE trace_versions
        ADD CONSTRAINT trace_versions_status_check
        CHECK (status IN ('DRAFT', 'FINALIZED', 'ARCHIVED'));
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'trace_versions_finalized_number_check'
          AND conrelid = 'trace_versions'::regclass
    ) THEN
        ALTER TABLE trace_versions
        ADD CONSTRAINT trace_versions_finalized_number_check
        CHECK (
            (status = 'FINALIZED' AND version_number IS NOT NULL AND finalized_at IS NOT NULL)
            OR status <> 'FINALIZED'
        );
    END IF;
END $$;

ALTER TABLE traces
ADD COLUMN IF NOT EXISTS current_version_id UUID NULL REFERENCES trace_versions(id) ON DELETE SET NULL;

ALTER TABLE posts
ADD COLUMN IF NOT EXISTS trace_version_id UUID NULL REFERENCES trace_versions(id) ON DELETE SET NULL,
ADD COLUMN IF NOT EXISTS content_source TEXT;

UPDATE posts
SET content_source = CASE
    WHEN source_trace_id IS NULL OR trace_version_id IS NULL THEN 'CUSTOM'
    ELSE 'TRACE_VERSION'
END
WHERE content_source IS NULL;

ALTER TABLE posts
ALTER COLUMN content_source SET DEFAULT 'TRACE_VERSION',
ALTER COLUMN content_source SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'posts_content_source_check'
          AND conrelid = 'posts'::regclass
    ) THEN
        ALTER TABLE posts
        ADD CONSTRAINT posts_content_source_check
        CHECK (content_source IN ('TRACE_VERSION', 'CUSTOM'));
    END IF;
END $$;

ALTER TABLE landscape_analyses
ADD COLUMN IF NOT EXISTS trace_version_id UUID NULL REFERENCES trace_versions(id) ON DELETE SET NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_trace_versions_trace_id_version_number_unique
ON trace_versions (trace_id, version_number)
WHERE version_number IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_trace_versions_unique_draft
ON trace_versions (trace_id)
WHERE status = 'DRAFT';

CREATE INDEX IF NOT EXISTS idx_trace_versions_trace_id_status
ON trace_versions (trace_id, status);

CREATE INDEX IF NOT EXISTS idx_traces_current_version_id
ON traces (current_version_id);

CREATE INDEX IF NOT EXISTS idx_posts_trace_version_id
ON posts (trace_version_id);

CREATE INDEX IF NOT EXISTS idx_landscape_analyses_trace_version_id
ON landscape_analyses (trace_version_id);
