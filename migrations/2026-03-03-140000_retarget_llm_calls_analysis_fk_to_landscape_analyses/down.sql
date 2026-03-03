ALTER TABLE llm_calls DROP CONSTRAINT IF EXISTS llm_calls_analysis_id_fkey;

-- Keep rollback robust: some analysis IDs may only exist in landscape_analyses.
-- Null them before restoring the FK to legacy resources.
UPDATE llm_calls
SET analysis_id = NULL
WHERE analysis_id IS NOT NULL
  AND analysis_id NOT IN (SELECT id FROM resources);

ALTER TABLE llm_calls
ADD CONSTRAINT llm_calls_analysis_id_fkey
FOREIGN KEY (analysis_id)
REFERENCES resources(id)
ON DELETE SET NULL;
