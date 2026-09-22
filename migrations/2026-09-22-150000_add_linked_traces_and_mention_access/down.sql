DROP INDEX IF EXISTS traces_linked_source_idx;
DROP INDEX IF EXISTS traces_journal_type_status_idx;
DROP INDEX IF EXISTS traces_linked_source_per_user_idx;

ALTER TABLE traces
    DROP CONSTRAINT IF EXISTS traces_linked_source_check,
    DROP CONSTRAINT IF EXISTS traces_trace_type_check;

ALTER TABLE traces
    ADD CONSTRAINT traces_trace_type_check CHECK (
        trace_type IN (
            'USER_TRACE',
            'BIO_TRACE',
            'WORKSPACE_TRACE',
            'HIGH_LEVEL_PROJECTS_DEFINITION'
        )
    ),
    DROP COLUMN IF EXISTS linked_source_trace_id;

DROP INDEX IF EXISTS trace_mentions_granted_access_idx;

ALTER TABLE trace_mentions
    DROP COLUMN IF EXISTS grants_trace_access;
