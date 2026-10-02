ALTER TABLE users ADD COLUMN shared_journal_weekly_digest_enabled BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE users SET shared_journal_weekly_digest_enabled = FALSE
WHERE shared_journal_activity_email_mode = 'off';

CREATE TABLE publication_push_deliveries (
    post_id UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    recipient_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    publishing_date TIMESTAMP NULL,
    sent_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (post_id, recipient_user_id)
);
CREATE INDEX publication_push_deliveries_recipient_idx
ON publication_push_deliveries (recipient_user_id, post_id);

ALTER TABLE notification_digests DROP CONSTRAINT notification_digests_kind_check;
ALTER TABLE notification_digests ADD CONSTRAINT notification_digests_kind_check
CHECK (digest_kind IN ('SHARED_JOURNAL_ACTIVITY_DAILY', 'SHARED_JOURNAL_ACTIVITY_WEEKLY'));
