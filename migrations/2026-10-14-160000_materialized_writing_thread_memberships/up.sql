CREATE TABLE writing_thread_traces (
    thread_id UUID NOT NULL REFERENCES writing_threads(id) ON DELETE CASCADE,
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position>=0),
    -- Cached contributions let edits apply deltas without reading other trace contents.
    is_counted BOOLEAN NOT NULL,
    character_count BIGINT NOT NULL CHECK (character_count>=0),
    finalized_at TIMESTAMP,
    PRIMARY KEY(thread_id,trace_id),
    UNIQUE(thread_id,position)
);
CREATE INDEX writing_thread_traces_trace_idx ON writing_thread_traces(trace_id,thread_id);
ALTER TABLE writing_threads ADD COLUMN trace_count BIGINT NOT NULL DEFAULT 0 CHECK (trace_count>=0);
ALTER TABLE writing_threads ADD COLUMN character_count BIGINT NOT NULL DEFAULT 0 CHECK (character_count>=0);
ALTER TABLE writing_threads ADD COLUMN last_activity_at TIMESTAMP;
CREATE INDEX writing_threads_activity_idx ON writing_threads(user_id,last_activity_at DESC,created_at DESC,id DESC);

-- Recursive traversal is confined to structural writes and explicit repair/backfill.
CREATE OR REPLACE FUNCTION attach_writing_thread_from_link() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE target UUID; downstream UUID;
BEGIN
    IF NOT EXISTS(SELECT 1 FROM users WHERE id=NEW.user_id) THEN RETURN NEW; END IF;
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


CREATE FUNCTION rebuild_writing_thread_memberships(target UUID) RETURNS VOID LANGUAGE plpgsql AS $$
DECLARE first_member UUID;
BEGIN
    PERFORM 1 FROM writing_threads w JOIN users u ON u.id=w.user_id WHERE w.id=target FOR UPDATE OF w;
    IF NOT FOUND THEN RETURN; END IF;
    DELETE FROM writing_thread_traces WHERE thread_id=target;
    INSERT INTO writing_thread_traces(thread_id,trace_id,position,is_counted,character_count,finalized_at)
    SELECT target,t.id,(row_number() OVER(ORDER BY c.depth DESC)-1)::integer,
        t.status='FINALIZED',
        CASE WHEN t.status='FINALIZED' AND NOT t.is_encrypted
            THEN char_length(t.title)::bigint+char_length(t.content)::bigint ELSE 0 END,
        CASE WHEN t.status='FINALIZED' THEN t.finalized_at ELSE NULL END
    FROM writing_thread_trace_ids(target) c JOIN traces t ON t.id=c.trace_id;
    UPDATE writing_threads w SET
        trace_count=(SELECT count(*) FROM writing_thread_traces WHERE thread_id=target AND is_counted),
        character_count=(SELECT COALESCE(sum(character_count),0) FROM writing_thread_traces WHERE thread_id=target),
        last_activity_at=(SELECT max(finalized_at) FROM writing_thread_traces WHERE thread_id=target AND is_counted)
    WHERE w.id=target;
    -- A physical deletion can sever the chain before its old starting point.
    SELECT trace_id INTO first_member FROM writing_thread_traces WHERE thread_id=target ORDER BY position LIMIT 1;
    IF first_member IS NOT NULL AND EXISTS(
        SELECT 1 FROM writing_threads w JOIN traces t ON t.id=w.start_trace_id WHERE w.id=target
    ) AND NOT EXISTS(
        SELECT 1 FROM writing_threads w JOIN writing_thread_traces m
            ON m.thread_id=w.id AND m.trace_id=w.start_trace_id WHERE w.id=target
    ) THEN
        UPDATE writing_threads SET start_trace_id=first_member WHERE id=target;
    END IF;
END $$;

CREATE FUNCTION refresh_writing_thread_structure() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM users WHERE id=NEW.user_id) THEN RETURN NEW; END IF;
    PERFORM rebuild_writing_thread_memberships(NEW.id);
    IF TG_OP='UPDATE' THEN
        UPDATE writing_threads SET input_revision=input_revision+1,
            description=NULL,title=NULL,ongoing_question=NULL,journey_summary=NULL,
            search_tags='{}',search_vector=''::tsvector,compiled_revision=NULL,
            generation_status='PENDING',generation_due_at=NOW()+INTERVAL '15 seconds',
            generation_claim_id=NULL,generation_lease_until=NULL,generation_error=NULL,updated_at=NOW()
        WHERE id=NEW.id;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER writing_thread_structure_insert AFTER INSERT ON writing_threads
FOR EACH ROW EXECUTE FUNCTION refresh_writing_thread_structure();
CREATE TRIGGER writing_thread_structure_update AFTER UPDATE OF start_trace_id,latest_trace_id ON writing_threads
FOR EACH ROW WHEN (OLD.start_trace_id IS DISTINCT FROM NEW.start_trace_id
    OR OLD.latest_trace_id IS DISTINCT FROM NEW.latest_trace_id)
EXECUTE FUNCTION refresh_writing_thread_structure();

CREATE OR REPLACE FUNCTION refresh_writing_threads_from_trace() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE target UUID; member writing_thread_traces%ROWTYPE; counted BOOLEAN; characters BIGINT;
BEGIN
    IF NOT EXISTS(SELECT 1 FROM users WHERE id=NEW.user_id) THEN RETURN NEW; END IF;
    IF NEW.status='FINALIZED' AND OLD.status='DRAFT' THEN
        UPDATE writing_threads SET latest_trace_id=NEW.id,pending_trace_id=NULL WHERE pending_trace_id=NEW.id;
    ELSIF NEW.status='ARCHIVED' AND OLD.status='DRAFT' THEN
        UPDATE writing_threads SET pending_trace_id=NULL WHERE pending_trace_id=NEW.id;
    END IF;
    IF NEW.status='DRAFT' AND OLD.status='DRAFT' THEN RETURN NEW; END IF;
    counted:=NEW.status='FINALIZED';
    characters:=CASE WHEN counted AND NOT NEW.is_encrypted
        THEN char_length(NEW.title)::bigint+char_length(NEW.content)::bigint ELSE 0 END;
    FOR target IN SELECT m.thread_id FROM writing_thread_traces m
        JOIN writing_threads w ON w.id=m.thread_id
        WHERE m.trace_id=NEW.id AND w.user_id=NEW.user_id ORDER BY m.thread_id
    LOOP
        -- Lock first, then re-read the contribution: a concurrent structural rebuild
        -- may have replaced the membership while this writer was waiting.
        PERFORM 1 FROM writing_threads WHERE id=target FOR UPDATE;
        IF NOT FOUND THEN CONTINUE; END IF;
        IF OLD.continues_trace_id IS DISTINCT FROM NEW.continues_trace_id
            AND NEW.continues_trace_id IS NULL AND OLD.status<>'DRAFT' THEN
            PERFORM rebuild_writing_thread_memberships(target);
        END IF;
        SELECT * INTO member FROM writing_thread_traces WHERE thread_id=target AND trace_id=NEW.id;
        IF NOT FOUND THEN CONTINUE; END IF;
        UPDATE writing_thread_traces SET is_counted=counted,character_count=characters,
            finalized_at=CASE WHEN counted THEN NEW.finalized_at ELSE NULL END
        WHERE thread_id=target AND trace_id=NEW.id;
        UPDATE writing_threads w SET
            trace_count=w.trace_count+counted::integer-member.is_counted::integer,
            character_count=w.character_count+characters-member.character_count,
            last_activity_at=CASE
                WHEN member.is_counted AND member.finalized_at IS NOT NULL
                    AND w.last_activity_at=member.finalized_at
                    AND (NOT counted OR NEW.finalized_at IS NULL OR NEW.finalized_at<member.finalized_at)
                THEN (SELECT max(finalized_at) FROM writing_thread_traces WHERE thread_id=target AND is_counted)
                WHEN counted THEN GREATEST(w.last_activity_at,NEW.finalized_at)
                ELSE w.last_activity_at END,
            input_revision=w.input_revision+1,description=NULL,title=NULL,ongoing_question=NULL,journey_summary=NULL,
            search_tags='{}',search_vector=''::tsvector,compiled_revision=NULL,
            generation_status='PENDING',generation_due_at=NOW()+INTERVAL '15 seconds',
            generation_claim_id=NULL,generation_lease_until=NULL,generation_error=NULL,updated_at=NOW()
        WHERE w.id=target;
    END LOOP;
    RETURN NEW;
END $$;

-- Physical deletion removes membership via FK cascades; subtract its cached
-- contribution before the trace disappears. Structural FK updates rebuild as needed.
CREATE FUNCTION remove_writing_thread_trace_contribution() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE target UUID; member writing_thread_traces%ROWTYPE;
BEGIN
    IF NOT EXISTS(SELECT 1 FROM users WHERE id=OLD.user_id) THEN RETURN OLD; END IF;
    FOR target IN SELECT thread_id FROM writing_thread_traces WHERE trace_id=OLD.id ORDER BY thread_id LOOP
        PERFORM 1 FROM writing_threads WHERE id=target FOR UPDATE;
        IF NOT FOUND THEN CONTINUE; END IF;
        SELECT * INTO member FROM writing_thread_traces WHERE thread_id=target AND trace_id=OLD.id;
        IF NOT FOUND THEN CONTINUE; END IF;
        UPDATE writing_threads w SET
            trace_count=w.trace_count-member.is_counted::integer,
            character_count=w.character_count-member.character_count,
            last_activity_at=CASE WHEN w.last_activity_at=member.finalized_at
                THEN (SELECT max(finalized_at) FROM writing_thread_traces
                    WHERE thread_id=target AND trace_id<>OLD.id AND is_counted)
                ELSE w.last_activity_at END,
            input_revision=w.input_revision+1,description=NULL,title=NULL,ongoing_question=NULL,journey_summary=NULL,
            search_tags='{}',search_vector=''::tsvector,compiled_revision=NULL,
            generation_status='PENDING',generation_due_at=NOW()+INTERVAL '15 seconds',
            generation_claim_id=NULL,generation_lease_until=NULL,generation_error=NULL,updated_at=NOW()
        WHERE w.id=target;
    END LOOP;
    RETURN OLD;
END $$;
CREATE TRIGGER writing_thread_trace_delete BEFORE DELETE ON traces
FOR EACH ROW EXECUTE FUNCTION remove_writing_thread_trace_contribution();

-- Backfill metrics/membership only; existing valid AI projections stay valid.
DO $$
DECLARE target UUID;
BEGIN
    FOR target IN SELECT id FROM writing_threads ORDER BY id LOOP
        PERFORM rebuild_writing_thread_memberships(target);
    END LOOP;
END $$;
