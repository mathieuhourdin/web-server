CREATE INDEX writing_threads_start_idx ON writing_threads(user_id,start_trace_id);

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

-- Thread records are disposable projections: repair split linear prefixes without
-- altering any trace links, content, sharing rights or pending draft associations.
DO $$
DECLARE upstream UUID; downstream UUID;
BEGIN
    LOOP
        SELECT u.id,d.id INTO upstream,downstream
        FROM writing_threads u JOIN writing_threads d
            ON d.user_id=u.user_id AND d.start_trace_id=u.latest_trace_id AND d.id<>u.id
        WHERE u.pending_trace_id IS NULL
        ORDER BY d.created_at,d.id,u.created_at,u.id
        LIMIT 1 FOR UPDATE OF u,d;
        EXIT WHEN NOT FOUND;
        UPDATE writing_threads SET
            start_trace_id=(SELECT start_trace_id FROM writing_threads WHERE id=upstream),
            input_revision=input_revision+1,description=NULL,search_tags='{}',search_vector=''::tsvector,
            compiled_revision=NULL,generation_status='PENDING',generation_due_at=NOW()+INTERVAL '15 seconds',
            generation_claim_id=NULL,generation_lease_until=NULL,generation_error=NULL,updated_at=NOW()
        WHERE id=downstream;
        DELETE FROM writing_threads WHERE id=upstream;
    END LOOP;
END $$;
