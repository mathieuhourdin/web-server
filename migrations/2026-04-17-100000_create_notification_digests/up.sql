CREATE TABLE IF NOT EXISTS notification_digests (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    recipient_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    digest_kind TEXT NOT NULL,
    local_date DATE NOT NULL,
    timezone TEXT NOT NULL,
    status TEXT NOT NULL,
    outbound_email_id UUID NULL REFERENCES outbound_emails(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT notification_digests_kind_check
        CHECK (digest_kind IN ('SHARED_JOURNAL_ACTIVITY_DAILY')),
    CONSTRAINT notification_digests_status_check
        CHECK (status IN ('EMPTY', 'ENQUEUED')),
    CONSTRAINT notification_digests_unique_key
        UNIQUE (recipient_user_id, digest_kind, local_date, timezone)
);

SELECT diesel_manage_updated_at('notification_digests');
