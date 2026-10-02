DROP TABLE publication_push_deliveries;
DELETE FROM notification_digests WHERE digest_kind = 'SHARED_JOURNAL_ACTIVITY_WEEKLY';
ALTER TABLE notification_digests DROP CONSTRAINT notification_digests_kind_check;
ALTER TABLE notification_digests ADD CONSTRAINT notification_digests_kind_check
CHECK (digest_kind = 'SHARED_JOURNAL_ACTIVITY_DAILY');
ALTER TABLE users DROP COLUMN shared_journal_weekly_digest_enabled;
