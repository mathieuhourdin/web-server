DROP TRIGGER a_trace_chronology ON traces;
DROP FUNCTION maintain_trace_chronology();
DROP INDEX traces_finalized_chronology_idx;
ALTER TABLE traces DROP COLUMN is_historical_import;
UPDATE traces SET start_writing_at=created_at WHERE start_writing_at IS NULL;
ALTER TABLE traces ALTER COLUMN start_writing_at SET NOT NULL;
