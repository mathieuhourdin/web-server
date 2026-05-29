ALTER TABLE journals
ADD COLUMN sharing_mode TEXT NOT NULL DEFAULT 'PRIVATE';

ALTER TABLE journals
ADD CONSTRAINT journals_sharing_mode_check
CHECK (sharing_mode IN ('PRIVATE', 'SEMI_SHARED', 'SHARED'));

UPDATE journals
SET sharing_mode = 'SEMI_SHARED'
FROM journal_grants
WHERE journal_grants.journal_id = journals.id;

UPDATE journals
SET sharing_mode = 'SHARED'
FROM journal_grants
WHERE journal_grants.journal_id = journals.id
AND journal_grants.grantee_scope = 'ALL_ACCEPTED_FOLLOWERS';
