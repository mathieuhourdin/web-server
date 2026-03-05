ALTER TABLE landscape_analyses
DROP CONSTRAINT IF EXISTS landscape_analyses_landscape_analysis_type_check;

ALTER TABLE landscape_analyses
ADD CONSTRAINT landscape_analyses_landscape_analysis_type_check
CHECK (
    landscape_analysis_type IN (
        'DAILY_RECAP',
        'TRACE_INCREMENTAL'
    )
);

ALTER TABLE landscape_analyses
DROP CONSTRAINT IF EXISTS landscape_analyses_processing_state_check;

ALTER TABLE landscape_analyses
ADD CONSTRAINT landscape_analyses_processing_state_check
CHECK (
    processing_state IN (
        'PENDING',
        'REPLAY_REQUESTED',
        'COMPLETED'
    )
);
