ALTER TABLE outbound_emails
    DROP CONSTRAINT outbound_emails_status_check;

ALTER TABLE outbound_emails
    ADD CONSTRAINT outbound_emails_status_check
    CHECK (status IN ('PENDING', 'RUNNING', 'SENT', 'FAILED'));

ALTER TABLE outbound_emails
    ADD COLUMN lock_owner UUID NULL,
    ADD COLUMN lock_until TIMESTAMP NULL;

CREATE INDEX outbound_emails_status_lock_until_scheduled_at_idx
ON outbound_emails (status, lock_until, scheduled_at, created_at);
