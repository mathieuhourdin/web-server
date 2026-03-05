DROP TRIGGER IF EXISTS trg_prevent_overlapping_lens_analysis_scopes ON lens_analysis_scopes;

DROP FUNCTION IF EXISTS prevent_overlapping_lens_analysis_scopes();

DROP INDEX IF EXISTS idx_landscape_analyses_type_period_end;
DROP INDEX IF EXISTS idx_landscape_analyses_processing_state_period_end;
