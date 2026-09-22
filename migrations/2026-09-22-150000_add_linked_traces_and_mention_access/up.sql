ALTER TABLE trace_mentions
    ADD COLUMN grants_trace_access BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX trace_mentions_granted_access_idx
    ON trace_mentions (mentioned_user_id, trace_id)
    WHERE removed_at IS NULL AND grants_trace_access = TRUE;

ALTER TABLE traces
    ADD COLUMN linked_source_trace_id UUID NULL REFERENCES traces(id) ON DELETE CASCADE;

ALTER TABLE traces
    DROP CONSTRAINT traces_trace_type_check;

ALTER TABLE traces
    ADD CONSTRAINT traces_trace_type_check CHECK (
        trace_type IN (
            'USER_TRACE',
            'BIO_TRACE',
            'WORKSPACE_TRACE',
            'HIGH_LEVEL_PROJECTS_DEFINITION',
            'LINKED_TRACE'
        )
    ),
    ADD CONSTRAINT traces_linked_source_check CHECK (
        (trace_type = 'LINKED_TRACE' AND linked_source_trace_id IS NOT NULL)
        OR
        (trace_type <> 'LINKED_TRACE' AND linked_source_trace_id IS NULL)
    );

CREATE UNIQUE INDEX traces_linked_source_per_user_idx
    ON traces (user_id, linked_source_trace_id)
    WHERE trace_type = 'LINKED_TRACE';

CREATE INDEX traces_journal_type_status_idx
    ON traces (journal_id, trace_type, status);

CREATE INDEX traces_linked_source_idx
    ON traces (linked_source_trace_id)
    WHERE linked_source_trace_id IS NOT NULL;
