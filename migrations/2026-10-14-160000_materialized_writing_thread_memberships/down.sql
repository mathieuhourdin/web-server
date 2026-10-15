CREATE OR REPLACE FUNCTION attach_writing_thread_from_link() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE target UUID; downstream UUID;
BEGIN
    IF TG_OP='UPDATE' AND OLD.continues_trace_id IS NOT DISTINCT FROM NEW.continues_trace_id THEN
        RETURN NEW;
    END IF;
    DELETE FROM writing_threads WHERE pending_trace_id=NEW.id AND latest_trace_id=start_trace_id;
    UPDATE writing_threads SET pending_trace_id=NULL WHERE pending_trace_id=NEW.id;
    IF NEW.continues_trace_id IS NULL THEN RETURN NEW; END IF;
    -- A newly linked finalized root may already head a downstream chain.
    -- Extend that chain backward, rather than creating a prefix-only thread.
    IF NEW.status='FINALIZED' THEN
        SELECT id INTO downstream FROM writing_threads
        WHERE user_id=NEW.user_id AND start_trace_id=NEW.id
        ORDER BY created_at,id LIMIT 1 FOR UPDATE;
        IF downstream IS NOT NULL THEN
            SELECT id INTO target FROM writing_threads
            WHERE user_id=NEW.user_id AND latest_trace_id=NEW.continues_trace_id
                AND pending_trace_id IS NULL AND id<>downstream
            ORDER BY created_at,id LIMIT 1 FOR UPDATE;
            IF target IS NOT NULL THEN
                UPDATE writing_threads SET start_trace_id=(
                    SELECT start_trace_id FROM writing_threads WHERE id=target
                ) WHERE id=downstream;
                DELETE FROM writing_threads WHERE id=target;
            ELSE
                UPDATE writing_threads SET start_trace_id=NEW.continues_trace_id WHERE id=downstream;
            END IF;
            RETURN NEW;
        END IF;
    END IF;
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


DROP TRIGGER writing_thread_trace_delete ON traces;
DROP TRIGGER writing_thread_structure_insert ON writing_threads;
DROP TRIGGER writing_thread_structure_update ON writing_threads;
CREATE OR REPLACE FUNCTION refresh_writing_threads_from_trace() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.status='FINALIZED' AND OLD.status='DRAFT' THEN
        UPDATE writing_threads SET latest_trace_id=NEW.id,pending_trace_id=NULL
        WHERE pending_trace_id=NEW.id;
    ELSIF NEW.status='ARCHIVED' AND OLD.status='DRAFT' THEN
        UPDATE writing_threads SET pending_trace_id=NULL WHERE pending_trace_id=NEW.id;
    END IF;
    -- A draft does not belong to the compiled chain yet and must not bump activity.
    IF NEW.status='DRAFT' AND OLD.status='DRAFT' THEN RETURN NEW; END IF;
    UPDATE writing_threads w SET
        input_revision=input_revision+1,description=NULL,title=NULL,ongoing_question=NULL,journey_summary=NULL,search_tags='{}',search_vector=''::tsvector,
        compiled_revision=NULL,generation_status='PENDING',generation_due_at=NOW()+INTERVAL '15 seconds',
        generation_claim_id=NULL,generation_lease_until=NULL,generation_error=NULL,updated_at=NOW()
    WHERE w.user_id=NEW.user_id AND EXISTS(SELECT 1 FROM writing_thread_trace_ids(w.id) c WHERE c.trace_id=NEW.id);
    RETURN NEW;
END $$;


DROP FUNCTION remove_writing_thread_trace_contribution();
DROP FUNCTION refresh_writing_thread_structure();
DROP FUNCTION rebuild_writing_thread_memberships(UUID);
DROP TABLE writing_thread_traces;
DROP INDEX writing_threads_activity_idx;
ALTER TABLE writing_threads DROP COLUMN trace_count;
ALTER TABLE writing_threads DROP COLUMN character_count;
ALTER TABLE writing_threads DROP COLUMN last_activity_at;
