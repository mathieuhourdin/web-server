DROP INDEX IF EXISTS traces_parent_trace_created_at_idx;

DELETE FROM posts
WHERE source_trace_id IN (
    SELECT id FROM traces WHERE trace_type = 'TRACE_COMPLEMENT'
);

DELETE FROM traces WHERE trace_type = 'TRACE_COMPLEMENT';

ALTER TABLE traces
    DROP CONSTRAINT IF EXISTS traces_parent_trace_not_self_check,
    DROP CONSTRAINT IF EXISTS traces_complement_audience_mode_check,
    DROP CONSTRAINT IF EXISTS traces_relationship_type_check,
    DROP CONSTRAINT IF EXISTS traces_trace_type_check;

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
    ),
    DROP COLUMN IF EXISTS parent_trace_id,
    DROP COLUMN IF EXISTS complement_audience_mode,
    ALTER COLUMN journal_id SET NOT NULL;
