CREATE TABLE IF NOT EXISTS content_reports (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    reporter_user_id UUID NOT NULL REFERENCES users(id),
    reported_message_id UUID NULL REFERENCES messages(id) ON DELETE SET NULL,
    reported_post_id UUID NULL REFERENCES posts(id) ON DELETE SET NULL,
    reported_user_id UUID NOT NULL REFERENCES users(id),
    reason TEXT NOT NULL CHECK (
        reason IN (
            'AGGRESSIVE',
            'HARASSMENT',
            'HATE',
            'SEXUAL',
            'SPAM',
            'SELF_HARM',
            'MISINFORMATION',
            'PRIVACY',
            'OTHER'
        )
    ),
    reporter_comment TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL CHECK (status IN ('OPEN', 'REVIEWED', 'DISMISSED', 'ACTION_TAKEN')),
    reviewed_by_user_id UUID NULL REFERENCES users(id),
    reviewed_at TIMESTAMP NULL,
    resolution_note TEXT NOT NULL DEFAULT '',
    snapshot_title TEXT NOT NULL DEFAULT '',
    snapshot_content TEXT NOT NULL DEFAULT '',
    snapshot_attachment_json JSONB NULL,
    snapshot_metadata_json JSONB NULL,
    snapshot_source_kind TEXT NULL CHECK (
        snapshot_source_kind IN ('MESSAGE', 'TRACE', 'DOCUMENT', 'ALBUM')
    ),
    snapshot_source_id UUID NULL,
    snapshot_context_json JSONB NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT content_reports_single_target_check CHECK (
        (reported_message_id IS NOT NULL) <> (reported_post_id IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_content_reports_reporter_user_id
ON content_reports (reporter_user_id);

CREATE INDEX IF NOT EXISTS idx_content_reports_reported_user_id
ON content_reports (reported_user_id);

CREATE INDEX IF NOT EXISTS idx_content_reports_reported_message_id
ON content_reports (reported_message_id);

CREATE INDEX IF NOT EXISTS idx_content_reports_reported_post_id
ON content_reports (reported_post_id);

CREATE INDEX IF NOT EXISTS idx_content_reports_status_created_at
ON content_reports (status, created_at DESC);
