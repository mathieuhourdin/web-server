-- Private, request-time input snapshots, not a new conversation/message type.
CREATE TABLE mentor_message_contexts (
    message_id UUID PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    input_json JSONB NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
