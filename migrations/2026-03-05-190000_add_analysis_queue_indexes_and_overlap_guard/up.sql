-- 1.4 Queue-oriented indexes
CREATE INDEX IF NOT EXISTS idx_landscape_analyses_processing_state_period_end
ON landscape_analyses (processing_state, period_end);

CREATE INDEX IF NOT EXISTS idx_landscape_analyses_type_period_end
ON landscape_analyses (landscape_analysis_type, period_end);

-- 1.5 Overlap guard for lens scope links
CREATE OR REPLACE FUNCTION prevent_overlapping_lens_analysis_scopes()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    candidate_type TEXT;
    candidate_period_start TIMESTAMP;
    candidate_period_end TIMESTAMP;
BEGIN
    SELECT
        la.landscape_analysis_type,
        la.period_start,
        la.period_end
    INTO
        candidate_type,
        candidate_period_start,
        candidate_period_end
    FROM landscape_analyses la
    WHERE la.id = NEW.landscape_analysis_id;

    IF candidate_period_start IS NULL OR candidate_period_end IS NULL THEN
        RETURN NEW;
    END IF;

    IF EXISTS (
        SELECT 1
        FROM lens_analysis_scopes las
        JOIN landscape_analyses existing_la ON existing_la.id = las.landscape_analysis_id
        WHERE las.lens_id = NEW.lens_id
          AND las.landscape_analysis_id <> NEW.landscape_analysis_id
          AND existing_la.landscape_analysis_type = candidate_type
          AND (
              (
                  existing_la.period_start = candidate_period_start
                  AND existing_la.period_end = candidate_period_end
              )
              OR (
                  existing_la.period_start < candidate_period_end
                  AND candidate_period_start < existing_la.period_end
              )
          )
    ) THEN
        RAISE EXCEPTION USING
            MESSAGE = format(
                'Overlapping analysis scope detected for lens %s and analysis type %s',
                NEW.lens_id,
                candidate_type
            );
    END IF;

    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_prevent_overlapping_lens_analysis_scopes ON lens_analysis_scopes;

CREATE TRIGGER trg_prevent_overlapping_lens_analysis_scopes
BEFORE INSERT OR UPDATE OF lens_id, landscape_analysis_id
ON lens_analysis_scopes
FOR EACH ROW
EXECUTE FUNCTION prevent_overlapping_lens_analysis_scopes();
