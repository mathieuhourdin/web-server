ALTER TABLE traces
    ALTER COLUMN journal_id DROP NOT NULL,
    ADD COLUMN parent_trace_id UUID NULL REFERENCES traces(id) ON DELETE CASCADE,
    ADD COLUMN complement_audience_mode TEXT NULL;

ALTER TABLE traces
    DROP CONSTRAINT traces_linked_source_check,
    DROP CONSTRAINT traces_trace_type_check;

ALTER TABLE traces
    ADD CONSTRAINT traces_trace_type_check CHECK (
        trace_type IN (
            'USER_TRACE',
            'BIO_TRACE',
            'WORKSPACE_TRACE',
            'HIGH_LEVEL_PROJECTS_DEFINITION',
            'LINKED_TRACE',
            'TRACE_COMPLEMENT'
        )
    ),
    ADD CONSTRAINT traces_relationship_type_check CHECK (
        (trace_type = 'LINKED_TRACE'
            AND linked_source_trace_id IS NOT NULL
            AND parent_trace_id IS NULL
            AND complement_audience_mode IS NULL
            AND journal_id IS NOT NULL)
        OR
        (trace_type = 'TRACE_COMPLEMENT'
            AND parent_trace_id IS NOT NULL
            AND linked_source_trace_id IS NULL
            AND complement_audience_mode IS NOT NULL
            AND journal_id IS NULL)
        OR
        (trace_type NOT IN ('LINKED_TRACE', 'TRACE_COMPLEMENT')
            AND linked_source_trace_id IS NULL
            AND parent_trace_id IS NULL
            AND complement_audience_mode IS NULL
            AND journal_id IS NOT NULL)
    ),
    ADD CONSTRAINT traces_complement_audience_mode_check CHECK (
        complement_audience_mode IS NULL
        OR complement_audience_mode IN ('INDEPENDENT', 'PARENT')
    ),
    ADD CONSTRAINT traces_parent_trace_not_self_check CHECK (
        parent_trace_id IS NULL OR parent_trace_id <> id
    );

CREATE INDEX traces_parent_trace_created_at_idx
    ON traces (parent_trace_id, created_at)
    WHERE trace_type = 'TRACE_COMPLEMENT';
