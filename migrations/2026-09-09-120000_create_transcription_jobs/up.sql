CREATE TABLE transcription_jobs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    source_asset_ids JSONB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('QUEUED','RUNNING','REVIEW','CONFIRMED','FAILED','CANCELLED')),
    pipeline TEXT NOT NULL,
    canonical_text TEXT,
    challenges JSONB NOT NULL DEFAULT '[]'::jsonb,
    estimated_cost_usd DOUBLE PRECISION,
    error_message TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    confirmed_at TIMESTAMP
);
CREATE INDEX transcription_jobs_trace_id_created_at_idx ON transcription_jobs(trace_id, created_at DESC);
