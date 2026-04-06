CREATE TABLE trace_attachments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    attachment_name TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX trace_attachments_trace_id_idx
    ON trace_attachments (trace_id, created_at ASC);

CREATE INDEX trace_attachments_asset_id_idx
    ON trace_attachments (asset_id);
