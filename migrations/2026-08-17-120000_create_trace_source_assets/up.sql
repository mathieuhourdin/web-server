CREATE TABLE trace_source_assets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT trace_source_assets_position_non_negative_check CHECK (position >= 0),
    CONSTRAINT trace_source_assets_trace_asset_unique UNIQUE (trace_id, asset_id),
    CONSTRAINT trace_source_assets_trace_position_unique UNIQUE (trace_id, position)
);

CREATE INDEX trace_source_assets_trace_id_position_idx
    ON trace_source_assets (trace_id, position ASC);

CREATE INDEX trace_source_assets_asset_id_idx
    ON trace_source_assets (asset_id);
