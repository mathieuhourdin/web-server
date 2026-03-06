CREATE TABLE IF NOT EXISTS analysis_summaries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    landscape_analysis_id UUID NOT NULL REFERENCES landscape_analyses(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    summary_type TEXT NOT NULL CHECK (summary_type IN ('PERIOD_RECAP')),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (landscape_analysis_id, summary_type)
);

CREATE INDEX IF NOT EXISTS idx_analysis_summaries_analysis_created_at
    ON analysis_summaries(landscape_analysis_id, created_at DESC);

CREATE TABLE IF NOT EXISTS messages (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    sender_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    landscape_analysis_id UUID NULL REFERENCES landscape_analyses(id) ON DELETE SET NULL,
    trace_id UUID NULL REFERENCES traces(id) ON DELETE SET NULL,
    message_type TEXT NOT NULL CHECK (message_type IN ('GENERAL', 'MENTOR_FEEDBACK')),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_messages_recipient_created_at
    ON messages(recipient_user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_messages_sender_created_at
    ON messages(sender_user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_messages_analysis_created_at
    ON messages(landscape_analysis_id, created_at DESC);
