DELETE FROM usage_events WHERE event_type='AI_WRITING_THREAD_COMPILATION_REQUESTED';
ALTER TABLE usage_events DROP CONSTRAINT usage_events_event_type_check;
ALTER TABLE usage_events ADD CONSTRAINT usage_events_event_type_check CHECK (event_type IN (
    'ACCOUNT_CREATED','ACCOUNT_DELETED','ACCOUNT_SELF_DELETED','ACCOUNT_ADMIN_DELETED','HOME_VISITED','HISTORY_VISITED',
    'FEED_VISITED','FEED_ENGAGED_30S','SHARED_JOURNAL_LANDING_VISITED','SHARED_JOURNAL_SIGNUP_CLICKED','JOURNAL_OPENED',
    'FOLLOWED_JOURNAL_OPENED','POST_OPENED','FEEDBACK_OPENED','SUMMARY_OPENED','TRACE_TIMEOUT_SET','TRACE_TIMEOUT_EXTENDED',
    'TRACE_TIMEOUT_AUTO_FINALIZED','AI_TRANSCRIPTION_REQUESTED','AI_WAL_COMPILATION_REQUESTED'
));
DROP TRIGGER attach_writing_thread_from_link ON traces;
DROP FUNCTION attach_writing_thread_from_link();
DROP TRIGGER refresh_writing_threads_from_trace ON traces;
DROP TRIGGER validate_trace_continuation ON traces;
DROP FUNCTION refresh_writing_threads_from_trace();
DROP FUNCTION validate_trace_continuation();
DROP FUNCTION writing_thread_trace_ids(UUID);
DROP TABLE writing_threads;
ALTER TABLE traces DROP COLUMN continues_trace_id;
