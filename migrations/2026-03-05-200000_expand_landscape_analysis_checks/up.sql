ALTER TABLE landscape_analyses
DROP CONSTRAINT IF EXISTS landscape_analyses_landscape_analysis_type_check;

ALTER TABLE landscape_analyses
ADD CONSTRAINT landscape_analyses_landscape_analysis_type_check
CHECK (
    landscape_analysis_type IN (
        'TRACE_INCREMENTAL',
        'DAILY_RECAP',
        'WEEKLY_RECAP',
        'HLP',
        'BIO'
    )
);

ALTER TABLE landscape_analyses
DROP CONSTRAINT IF EXISTS landscape_analyses_processing_state_check;

ALTER TABLE landscape_analyses
ADD CONSTRAINT landscape_analyses_processing_state_check
CHECK (
    processing_state IN (
        'PENDING',
        'RUNNING',
        'REPLAY_REQUESTED',
        'COMPLETED',
        'FAILED'
    )
);
