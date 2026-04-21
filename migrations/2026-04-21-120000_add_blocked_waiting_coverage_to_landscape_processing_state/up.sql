ALTER TABLE landscape_analyses
DROP CONSTRAINT IF EXISTS landscape_analyses_processing_state_check;

ALTER TABLE landscape_analyses
ADD CONSTRAINT landscape_analyses_processing_state_check
CHECK (
    processing_state IN (
        'PENDING',
        'BLOCKED_WAITING_COVERAGE',
        'RUNNING',
        'REPLAY_REQUESTED',
        'COMPLETED',
        'FAILED'
    )
);
