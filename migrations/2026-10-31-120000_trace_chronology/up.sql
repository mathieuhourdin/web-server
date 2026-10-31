ALTER TABLE traces ALTER COLUMN start_writing_at DROP NOT NULL;
ALTER TABLE traces ALTER COLUMN start_writing_at DROP DEFAULT;
ALTER TABLE traces ADD COLUMN is_historical_import BOOLEAN NOT NULL DEFAULT FALSE;

-- No reconstruction of old writing sessions. Keep existing completed dates.
ALTER TABLE traces DISABLE TRIGGER set_updated_at;
UPDATE traces SET start_writing_at=NULL WHERE status='DRAFT' AND is_blank;
UPDATE traces SET finalized_at=COALESCE(start_writing_at,interaction_date,created_at)
WHERE status='FINALIZED' AND finalized_at IS NULL;
UPDATE traces SET interaction_date=COALESCE(finalized_at,start_writing_at,created_at);
ALTER TABLE traces ENABLE TRIGGER set_updated_at;

ALTER TABLE journals DISABLE TRIGGER set_updated_at;
UPDATE journals j SET last_trace_at=(
    SELECT MAX(COALESCE(t.finalized_at,t.start_writing_at)) FROM traces t
    WHERE t.journal_id=j.id AND (t.status<>'DRAFT' OR t.trace_type<>'USER_TRACE' OR NOT t.is_blank)
);
ALTER TABLE journals ENABLE TRIGGER set_updated_at;

-- Compatibility column only: old readers can still consume interaction_date,
-- but clients can no longer move the activity clock by writing it.
CREATE FUNCTION maintain_trace_chronology() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.status='DRAFT' THEN
        NEW.finalized_at := NULL;
        IF NEW.is_blank THEN NEW.start_writing_at := NULL;
        ELSE NEW.start_writing_at := COALESCE(NEW.start_writing_at,NOW() AT TIME ZONE 'UTC');
        END IF;
    ELSE
        IF NEW.status='FINALIZED' THEN
            NEW.finalized_at := COALESCE(NEW.finalized_at,NOW() AT TIME ZONE 'UTC');
        END IF;
        IF NEW.finalized_at IS NOT NULL THEN
            NEW.start_writing_at := COALESCE(NEW.start_writing_at,NEW.finalized_at);
        END IF;
    END IF;
    NEW.interaction_date := COALESCE(NEW.finalized_at,NEW.start_writing_at,NEW.created_at,NOW() AT TIME ZONE 'UTC');
    RETURN NEW;
END;
$$;
CREATE TRIGGER a_trace_chronology BEFORE INSERT OR UPDATE ON traces
FOR EACH ROW EXECUTE FUNCTION maintain_trace_chronology();

CREATE INDEX traces_finalized_chronology_idx ON traces(user_id,finalized_at,id)
WHERE status='FINALIZED';

-- Preserve completed analyses and messages. Only unprocessed trace jobs move
-- to the canonical activity clock; pending recap coverage is replanned by the
-- existing internal replan endpoint after deployment.
UPDATE landscape_analyses a SET interaction_date=t.finalized_at,
    period_start=t.finalized_at,period_end=t.finalized_at
FROM traces t WHERE a.analyzed_trace_id=t.id AND t.finalized_at IS NOT NULL
AND a.landscape_analysis_type='TRACE_INCREMENTAL'
AND a.processing_state IN ('PENDING','BLOCKED_WAITING_COVERAGE','FAILED');

UPDATE trace_search_documents s SET interaction_date=t.finalized_at
FROM traces t WHERE t.id=s.trace_id AND t.finalized_at IS NOT NULL;
