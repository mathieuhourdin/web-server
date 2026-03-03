BEGIN;

ALTER TABLE llm_calls DROP CONSTRAINT IF EXISTS llm_calls_analysis_id_fkey;

ALTER TABLE llm_calls
ADD CONSTRAINT llm_calls_analysis_id_fkey
FOREIGN KEY (analysis_id)
REFERENCES resources(id)
ON DELETE SET NULL;

COMMIT;
