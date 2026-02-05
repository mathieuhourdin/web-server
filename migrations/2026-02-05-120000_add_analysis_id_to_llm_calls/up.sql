-- Add analysis_id to llm_calls, referencing resources (landscape_analysis)
ALTER TABLE llm_calls ADD COLUMN analysis_id UUID REFERENCES resources(id);

UPDATE llm_calls SET analysis_id = (SELECT id FROM resources WHERE entity_type = 'lnds' ORDER BY created_at DESC LIMIT 1);
ALTER TABLE llm_calls ALTER COLUMN analysis_id SET NOT NULL;