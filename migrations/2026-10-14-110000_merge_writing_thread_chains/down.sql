-- Merged projection rows are not reconstructed; trace links are unchanged.
DROP INDEX writing_threads_start_idx;

CREATE OR REPLACE FUNCTION attach_writing_thread_from_link() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE target UUID;
BEGIN
    IF TG_OP='UPDATE' AND OLD.continues_trace_id IS NOT DISTINCT FROM NEW.continues_trace_id THEN
        RETURN NEW;
    END IF;
    DELETE FROM writing_threads WHERE pending_trace_id=NEW.id AND latest_trace_id=start_trace_id;
    UPDATE writing_threads SET pending_trace_id=NULL WHERE pending_trace_id=NEW.id;
    IF NEW.continues_trace_id IS NULL THEN RETURN NEW; END IF;
    -- The validator locks the source, serializing concurrent continuations.
    SELECT id INTO target FROM writing_threads
    WHERE user_id=NEW.user_id AND latest_trace_id=NEW.continues_trace_id
        AND pending_trace_id IS NULL
    ORDER BY created_at,id LIMIT 1 FOR UPDATE;
    IF target IS NULL THEN
        INSERT INTO writing_threads(user_id,start_trace_id,latest_trace_id,pending_trace_id)
        VALUES(NEW.user_id,NEW.continues_trace_id,
            CASE WHEN NEW.status='FINALIZED' THEN NEW.id ELSE NEW.continues_trace_id END,
            CASE WHEN NEW.status='DRAFT' THEN NEW.id ELSE NULL END);
    ELSE
        UPDATE writing_threads SET
            latest_trace_id=CASE WHEN NEW.status='FINALIZED' THEN NEW.id ELSE latest_trace_id END,
            pending_trace_id=CASE WHEN NEW.status='DRAFT' THEN NEW.id ELSE NULL END
        WHERE id=target;
    END IF;
    RETURN NEW;
END $$;
