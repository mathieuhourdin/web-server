CREATE TABLE landscape_analysis_inputs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    landscape_analysis_id UUID NOT NULL REFERENCES landscape_analyses(id) ON DELETE CASCADE,
    trace_id UUID NULL REFERENCES traces(id) ON DELETE CASCADE,
    trace_mirror_id UUID NULL REFERENCES trace_mirrors(id) ON DELETE CASCADE,
    input_type TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT landscape_analysis_inputs_input_type_check
        CHECK (input_type = ANY (ARRAY['PRIMARY'::text, 'COVERED'::text])),
    CONSTRAINT landscape_analysis_inputs_source_check
        CHECK (trace_id IS NOT NULL OR trace_mirror_id IS NOT NULL)
);

CREATE UNIQUE INDEX uq_landscape_analysis_inputs_trace
ON landscape_analysis_inputs (landscape_analysis_id, trace_id, input_type)
WHERE trace_id IS NOT NULL;

CREATE UNIQUE INDEX uq_landscape_analysis_inputs_trace_mirror
ON landscape_analysis_inputs (landscape_analysis_id, trace_mirror_id, input_type)
WHERE trace_mirror_id IS NOT NULL;

CREATE INDEX idx_landscape_analysis_inputs_analysis
ON landscape_analysis_inputs (landscape_analysis_id);

CREATE INDEX idx_landscape_analysis_inputs_trace
ON landscape_analysis_inputs (trace_id)
WHERE trace_id IS NOT NULL;

CREATE INDEX idx_landscape_analysis_inputs_trace_mirror
ON landscape_analysis_inputs (trace_mirror_id)
WHERE trace_mirror_id IS NOT NULL;
