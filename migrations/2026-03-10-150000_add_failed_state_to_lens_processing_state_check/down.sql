UPDATE lenses
SET processing_state = 'OUT_OF_SYNC'
WHERE processing_state = 'FAILED';

ALTER TABLE lenses
DROP CONSTRAINT IF EXISTS lenses_processing_state_check;

ALTER TABLE lenses
ADD CONSTRAINT lenses_processing_state_check
CHECK (processing_state IN ('OUT_OF_SYNC', 'IN_SYNC'));
