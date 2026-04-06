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
        'SUMMARY_OPENED'
    )
);

DROP INDEX IF EXISTS traces_timeout_at_idx;

ALTER TABLE traces
DROP COLUMN IF EXISTS timeout_at;
