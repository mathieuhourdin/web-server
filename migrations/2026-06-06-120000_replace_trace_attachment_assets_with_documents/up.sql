DROP TABLE IF EXISTS trace_attachments;

CREATE TABLE trace_attachments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    attachment_name TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_trace_attachments_trace_id
    ON trace_attachments(trace_id);

CREATE INDEX idx_trace_attachments_document_id
    ON trace_attachments(document_id);
