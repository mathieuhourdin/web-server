CREATE TABLE trace_plumes (
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (trace_id, user_id)
);
CREATE INDEX trace_plumes_user_id_idx ON trace_plumes(user_id);
