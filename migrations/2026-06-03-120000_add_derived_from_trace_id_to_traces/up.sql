ALTER TABLE traces
ADD COLUMN derived_from_trace_id UUID NULL REFERENCES traces(id) ON DELETE SET NULL;
