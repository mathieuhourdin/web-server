ALTER TABLE journals
ADD CONSTRAINT journals_active_user_journal_current_draft_check
CHECK (
    status <> 'ACTIVE'
    OR journal_type <> 'USER_JOURNAL'
    OR current_draft_id IS NOT NULL
);
