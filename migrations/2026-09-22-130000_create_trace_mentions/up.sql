CREATE TABLE trace_mentions (
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    mentioned_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notified_at TIMESTAMP NULL,
    removed_at TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (trace_id, mentioned_user_id)
);

CREATE INDEX trace_mentions_active_trace_idx
    ON trace_mentions (trace_id)
    WHERE removed_at IS NULL;

CREATE INDEX trace_mentions_active_user_idx
    ON trace_mentions (mentioned_user_id)
    WHERE removed_at IS NULL;
