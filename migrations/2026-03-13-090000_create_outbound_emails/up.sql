CREATE TABLE outbound_emails (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    recipient_user_id UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    reason TEXT NOT NULL,
    resource_type TEXT NULL,
    resource_id UUID NULL,
    to_email TEXT NOT NULL,
    from_email TEXT NOT NULL,
    subject TEXT NOT NULL,
    text_body TEXT NULL,
    html_body TEXT NULL,
    status TEXT NOT NULL CHECK (status IN ('PENDING', 'SENT', 'FAILED')),
    provider TEXT NOT NULL CHECK (provider IN ('RESEND')),
    provider_message_id TEXT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT NULL,
    scheduled_at TIMESTAMP NULL,
    sent_at TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX outbound_emails_status_scheduled_at_idx
ON outbound_emails (status, scheduled_at, created_at);

CREATE INDEX outbound_emails_recipient_user_id_idx
ON outbound_emails (recipient_user_id);

CREATE INDEX outbound_emails_resource_idx
ON outbound_emails (resource_type, resource_id);
