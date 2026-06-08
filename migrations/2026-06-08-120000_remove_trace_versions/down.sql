CREATE TABLE trace_versions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    version_number INT4 NULL,
    status TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    subtitle TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    content_image_asset_id UUID NULL REFERENCES assets(id) ON DELETE SET NULL,
    interaction_date TIMESTAMP NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    finalized_at TIMESTAMP NULL,
    sharing_sensitivity TEXT NOT NULL DEFAULT 'NORMAL'
);

ALTER TABLE traces
ADD COLUMN IF NOT EXISTS current_version_id UUID NULL REFERENCES trace_versions(id) ON DELETE SET NULL;

ALTER TABLE posts
ADD COLUMN IF NOT EXISTS trace_version_id UUID NULL REFERENCES trace_versions(id) ON DELETE SET NULL;

ALTER TABLE landscape_analyses
ADD COLUMN IF NOT EXISTS trace_version_id UUID NULL REFERENCES trace_versions(id) ON DELETE SET NULL;
