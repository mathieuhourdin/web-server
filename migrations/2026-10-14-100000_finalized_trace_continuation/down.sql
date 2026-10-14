CREATE OR REPLACE FUNCTION validate_trace_continuation() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE source traces%ROWTYPE;
BEGIN
    IF TG_OP='UPDATE' AND OLD.continues_trace_id IS DISTINCT FROM NEW.continues_trace_id
        AND OLD.status<>'DRAFT'
        AND (NEW.continues_trace_id IS NOT NULL OR EXISTS(SELECT 1 FROM traces WHERE id=OLD.continues_trace_id)) THEN
        RAISE EXCEPTION 'Only drafts can change continuation links' USING ERRCODE='23514';
    END IF;
    IF NEW.continues_trace_id IS NOT NULL THEN
        SELECT * INTO source FROM traces WHERE id=NEW.continues_trace_id FOR UPDATE;
        IF NOT FOUND OR source.id=NEW.id OR source.user_id<>NEW.user_id
            OR source.journal_id IS DISTINCT FROM NEW.journal_id OR NEW.journal_id IS NULL
            OR source.trace_type<>'USER_TRACE' OR NEW.trace_type<>'USER_TRACE' THEN
            RAISE EXCEPTION 'Continuations require the same owner and journal' USING ERRCODE='23514';
        END IF;
        IF TG_OP='INSERT' OR (TG_OP='UPDATE' AND OLD.continues_trace_id IS DISTINCT FROM NEW.continues_trace_id) THEN
            IF NOT EXISTS(SELECT 1 FROM journals WHERE id=NEW.journal_id AND user_id=NEW.user_id
                AND status='ACTIVE' AND journal_type='USER_JOURNAL' AND NOT is_encrypted) THEN
                RAISE EXCEPTION 'Continue within an active unencrypted user journal' USING ERRCODE='23514';
            END IF;
            IF NEW.status NOT IN ('DRAFT','FINALIZED') OR NEW.is_encrypted OR source.status<>'FINALIZED' OR source.is_encrypted THEN
                RAISE EXCEPTION 'Continue a finalized unencrypted trace through a draft' USING ERRCODE='23514';
            END IF;
            IF TG_OP='UPDATE' AND OLD.status<>'DRAFT' THEN
                RAISE EXCEPTION 'Only drafts can acquire continuation links' USING ERRCODE='23514';
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
        AND (child.user_id<>NEW.user_id OR child.journal_id IS DISTINCT FROM NEW.journal_id OR NEW.trace_type<>'USER_TRACE')) THEN
        RAISE EXCEPTION 'Cannot move a trace out of its continuation chain' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

DROP TRIGGER refresh_writing_threads_from_trace ON traces;
CREATE TRIGGER refresh_writing_threads_from_trace AFTER UPDATE OF title,subtitle,content,status,is_encrypted,encryption_metadata,finalized_at ON traces
FOR EACH ROW WHEN (OLD.title IS DISTINCT FROM NEW.title OR OLD.subtitle IS DISTINCT FROM NEW.subtitle
    OR OLD.content IS DISTINCT FROM NEW.content OR OLD.status IS DISTINCT FROM NEW.status
    OR OLD.is_encrypted IS DISTINCT FROM NEW.is_encrypted OR OLD.encryption_metadata IS DISTINCT FROM NEW.encryption_metadata
    OR OLD.finalized_at IS DISTINCT FROM NEW.finalized_at)
EXECUTE FUNCTION refresh_writing_threads_from_trace();
