-- Keep continuation ownership and eligibility checks, but allow different journals.
-- Thread memberships already model chains independently of journals.
CREATE OR REPLACE FUNCTION validate_trace_continuation() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE source traces%ROWTYPE;
BEGIN
    IF TG_OP='UPDATE' AND OLD.continues_trace_id IS DISTINCT FROM NEW.continues_trace_id
        AND OLD.status<>'DRAFT'
        AND NOT (OLD.status='FINALIZED' AND OLD.continues_trace_id IS NULL AND NEW.continues_trace_id IS NOT NULL)
        AND (NEW.continues_trace_id IS NOT NULL OR EXISTS(SELECT 1 FROM traces WHERE id=OLD.continues_trace_id)) THEN
        RAISE EXCEPTION 'Finalized continuation links cannot be changed' USING ERRCODE='23514';
    END IF;
    IF NEW.continues_trace_id IS NOT NULL THEN
        SELECT * INTO source FROM traces WHERE id=NEW.continues_trace_id FOR UPDATE;
        IF NOT FOUND OR source.id=NEW.id OR source.user_id<>NEW.user_id
            OR source.journal_id IS NULL OR NEW.journal_id IS NULL
            OR source.trace_type<>'USER_TRACE' OR NEW.trace_type<>'USER_TRACE' THEN
            RAISE EXCEPTION 'Continuations require the same owner and ordinary journal traces' USING ERRCODE='23514';
        END IF;
        IF TG_OP='INSERT' OR (TG_OP='UPDATE' AND OLD.continues_trace_id IS DISTINCT FROM NEW.continues_trace_id) THEN
            IF NOT EXISTS(SELECT 1 FROM journals WHERE id=NEW.journal_id AND user_id=NEW.user_id
                AND status='ACTIVE' AND journal_type='USER_JOURNAL' AND NOT is_encrypted)
                OR NOT EXISTS(SELECT 1 FROM journals WHERE id=source.journal_id AND user_id=NEW.user_id
                AND status='ACTIVE' AND journal_type='USER_JOURNAL' AND NOT is_encrypted) THEN
                RAISE EXCEPTION 'Continue between active unencrypted user journals' USING ERRCODE='23514';
            END IF;
            IF NEW.status NOT IN ('DRAFT','FINALIZED') OR NEW.is_encrypted OR source.status<>'FINALIZED' OR source.is_encrypted THEN
                RAISE EXCEPTION 'Continuation endpoints must be eligible unencrypted traces' USING ERRCODE='23514';
            END IF;
            IF TG_OP='UPDATE' AND OLD.status<>'DRAFT'
                AND NOT (OLD.status='FINALIZED' AND OLD.continues_trace_id IS NULL) THEN
                RAISE EXCEPTION 'Only drafts or unlinked finalized traces can acquire continuation links' USING ERRCODE='23514';
            END IF;
            IF EXISTS(WITH RECURSIVE ancestors AS (
                SELECT id,continues_trace_id FROM traces WHERE id=NEW.continues_trace_id
                UNION SELECT t.id,t.continues_trace_id FROM traces t JOIN ancestors a ON t.id=a.continues_trace_id
            ) SELECT 1 FROM ancestors WHERE id=NEW.id) THEN
                RAISE EXCEPTION 'Continuation cycles are forbidden' USING ERRCODE='23514';
            END IF;
        END IF;
    END IF;
    IF EXISTS(SELECT 1 FROM traces child WHERE child.continues_trace_id=NEW.id
        AND (child.user_id<>NEW.user_id OR NEW.journal_id IS NULL OR NEW.trace_type<>'USER_TRACE')) THEN
        RAISE EXCEPTION 'Cannot change the owner or kind of a continuation chain trace' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
