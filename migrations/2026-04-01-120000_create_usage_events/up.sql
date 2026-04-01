CREATE TABLE usage_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id),
    session_id UUID REFERENCES sessions(id),
    event_type TEXT NOT NULL,
    resource_id UUID,
    context_json JSONB,
    occurred_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT usage_events_event_type_check CHECK (
        event_type IN (
            'HOME_VISITED',
            'HISTORY_VISITED',
            'JOURNAL_OPENED',
            'FOLLOWED_JOURNAL_OPENED',
            'POST_OPENED',
            'FEEDBACK_OPENED',
            'SUMMARY_OPENED'
        )
    )
);

CREATE INDEX usage_events_user_id_occurred_at_idx
    ON usage_events (user_id, occurred_at DESC);

CREATE INDEX usage_events_event_type_occurred_at_idx
    ON usage_events (event_type, occurred_at DESC);

CREATE INDEX usage_events_resource_id_idx
    ON usage_events (resource_id);
