-- Opting out of AI is not a pipeline failure. Match the exact historical guard
-- reason only: do not clear quota, overlap, provider or unexplained failures.
-- Pending work stays dormant while AI is disabled (the queue checks both flags).
UPDATE landscape_analyses
SET processing_state='PENDING', failure_reason=NULL, updated_at=NOW()
WHERE processing_state='FAILED'
  AND failure_reason='AI features are disabled for this account';

UPDATE lenses
SET processing_state='OUT_OF_SYNC', failure_reason=NULL,
    automatic_retry_at=NULL, automatic_retry_count=0, updated_at=NOW()
WHERE processing_state='FAILED'
  AND failure_reason='AI features are disabled for this account';
