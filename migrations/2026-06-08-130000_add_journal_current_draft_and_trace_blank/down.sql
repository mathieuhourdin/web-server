DROP INDEX IF EXISTS idx_journals_current_draft_id;
DROP INDEX IF EXISTS idx_traces_unique_user_draft_per_journal;

ALTER TABLE journals
DROP CONSTRAINT IF EXISTS journals_current_draft_id_fkey;

ALTER TABLE journals
DROP CONSTRAINT IF EXISTS journals_active_user_journal_current_draft_check;

ALTER TABLE journals
DROP CONSTRAINT IF EXISTS journals_journal_type_check;

ALTER TABLE journals
ADD CONSTRAINT journals_journal_type_check
CHECK (journal_type IN ('META_JOURNAL', 'WORK_LOG_JOURNAL', 'READING_NOTE_JOURNAL'));

ALTER TABLE journals
DROP CONSTRAINT IF EXISTS journals_status_check;

ALTER TABLE journals
ADD CONSTRAINT journals_status_check
CHECK (status IN ('DRAFT', 'PUBLISHED', 'ARCHIVED'));

UPDATE journals
SET status = CASE
    WHEN status = 'ARCHIVED' THEN 'ARCHIVED'
    ELSE 'PUBLISHED'
END;

UPDATE journals
SET journal_type = CASE
    WHEN journal_type = 'META_JOURNAL' THEN 'META_JOURNAL'
    ELSE 'WORK_LOG_JOURNAL'
END;

ALTER TABLE journals
DROP COLUMN IF EXISTS current_draft_id;

ALTER TABLE traces
DROP COLUMN IF EXISTS is_blank;
