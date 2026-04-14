DROP INDEX IF EXISTS outbound_emails_status_lock_until_scheduled_at_idx;

ALTER TABLE outbound_emails
    DROP COLUMN IF EXISTS lock_owner,
    DROP COLUMN IF EXISTS lock_until;

ALTER TABLE outbound_emails
    DROP CONSTRAINT outbound_emails_status_check;

ALTER TABLE outbound_emails
    ADD CONSTRAINT outbound_emails_status_check
    CHECK (status IN ('PENDING', 'SENT', 'FAILED'));
