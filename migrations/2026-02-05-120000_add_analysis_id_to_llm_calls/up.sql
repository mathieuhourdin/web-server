-- Add analysis_id to llm_calls, referencing resources (landscape_analysis)
ALTER TABLE llm_calls ADD COLUMN analysis_id UUID REFERENCES resources(id) ON DELETE SET NULL;