ALTER TABLE traces
ADD COLUMN timeout_at TIMESTAMP NULL;

CREATE INDEX traces_timeout_at_idx
    ON traces (timeout_at)
    WHERE status = 'DRAFT' AND timeout_at IS NOT NULL;

ALTER TABLE usage_events
DROP CONSTRAINT IF EXISTS usage_events_event_type_check;

ALTER TABLE usage_events
ADD CONSTRAINT usage_events_event_type_check CHECK (
    event_type IN (
        'HOME_VISITED',
        'HISTORY_VISITED',
        'JOURNAL_OPENED',
        'FOLLOWED_JOURNAL_OPENED',
        'POST_OPENED',
        'FEEDBACK_OPENED',
        'SUMMARY_OPENED',
        'TRACE_TIMEOUT_SET',
        'TRACE_TIMEOUT_EXTENDED',
        'TRACE_TIMEOUT_AUTO_FINALIZED'
    )
);
