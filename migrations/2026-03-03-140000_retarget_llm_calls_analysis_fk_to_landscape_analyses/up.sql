ALTER TABLE llm_calls DROP CONSTRAINT IF EXISTS llm_calls_analysis_id_fkey;

-- Some historical rows may contain analysis IDs that are not landscape analyses.
-- Keep migration robust by nulling those values before adding the new FK.
UPDATE llm_calls
SET analysis_id = NULL
WHERE analysis_id IS NOT NULL
  AND analysis_id NOT IN (SELECT id FROM landscape_analyses);

ALTER TABLE llm_calls
ADD CONSTRAINT llm_calls_analysis_id_fkey
FOREIGN KEY (analysis_id)
REFERENCES landscape_analyses(id)
ON DELETE SET NULL;
