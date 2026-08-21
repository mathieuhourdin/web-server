ALTER TABLE usage_events
    ALTER COLUMN user_id DROP NOT NULL;

ALTER TABLE usage_events
    DROP CONSTRAINT IF EXISTS usage_events_user_id_fkey;

ALTER TABLE usage_events
    ADD CONSTRAINT usage_events_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE usage_events
    DROP CONSTRAINT IF EXISTS usage_events_event_type_check;

ALTER TABLE usage_events
    ADD CONSTRAINT usage_events_event_type_check CHECK (
        event_type IN (
            'ACCOUNT_CREATED',
            'ACCOUNT_DELETED',
            'HOME_VISITED',
            'HISTORY_VISITED',
            'FEED_VISITED',
            'FEED_ENGAGED_30S',
            'SHARED_JOURNAL_LANDING_VISITED',
            'SHARED_JOURNAL_SIGNUP_CLICKED',
            'JOURNAL_OPENED',
            'FOLLOWED_JOURNAL_OPENED',
            'POST_OPENED',
            'FEEDBACK_OPENED',
            'SUMMARY_OPENED',
            'TRACE_TIMEOUT_SET',
            'TRACE_TIMEOUT_EXTENDED',
            'TRACE_TIMEOUT_AUTO_FINALIZED',
            'AI_TRANSCRIPTION_REQUESTED'
        )
    );
