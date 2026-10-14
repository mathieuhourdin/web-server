ALTER TABLE traces ADD COLUMN continues_trace_id UUID REFERENCES traces(id) ON DELETE SET NULL;
CREATE INDEX traces_continues_trace_idx ON traces(continues_trace_id) WHERE continues_trace_id IS NOT NULL;
CREATE TABLE writing_threads (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    start_trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    latest_trace_id UUID REFERENCES traces(id) ON DELETE SET NULL,
    pending_trace_id UUID REFERENCES traces(id) ON DELETE SET NULL,
    description TEXT,
    search_tags TEXT[] NOT NULL DEFAULT '{}',
    search_vector TSVECTOR NOT NULL DEFAULT ''::tsvector,
    input_revision BIGINT NOT NULL DEFAULT 1,
    compiled_revision BIGINT,
    generation_status TEXT NOT NULL DEFAULT 'PENDING' CHECK (generation_status IN ('PENDING','RUNNING','READY','FAILED','DISABLED')),
    generation_due_at TIMESTAMP NOT NULL DEFAULT NOW()+INTERVAL '15 seconds',
    generation_claim_id UUID,
    generation_lease_until TIMESTAMP,
    generation_error TEXT,
    generated_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE INDEX writing_threads_owner_idx ON writing_threads(user_id,latest_trace_id);
CREATE UNIQUE INDEX writing_threads_pending_idx ON writing_threads(pending_trace_id) WHERE pending_trace_id IS NOT NULL;
CREATE INDEX writing_threads_search_idx ON writing_threads USING GIN(search_vector);
CREATE INDEX writing_threads_due_idx ON writing_threads(generation_due_at) WHERE generation_status IN ('PENDING','RUNNING');

CREATE FUNCTION writing_thread_trace_ids(thread UUID) RETURNS TABLE(trace_id UUID,depth INTEGER)
LANGUAGE SQL STABLE AS $$
    WITH RECURSIVE chain AS (
        SELECT t.id,t.continues_trace_id,w.start_trace_id,0 AS depth,ARRAY[t.id] AS visited
        FROM writing_threads w JOIN traces t ON t.id=w.latest_trace_id WHERE w.id=thread
        UNION ALL
        SELECT t.id,t.continues_trace_id,c.start_trace_id,c.depth+1,c.visited||t.id
        FROM chain c JOIN traces t ON t.id=c.continues_trace_id
        WHERE c.id<>c.start_trace_id AND NOT t.id=ANY(c.visited)
    ) SELECT id,depth FROM chain
$$;

CREATE FUNCTION validate_trace_continuation() RETURNS TRIGGER LANGUAGE plpgsql AS $$
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
CREATE TRIGGER validate_trace_continuation BEFORE INSERT OR UPDATE OF continues_trace_id,user_id,journal_id,trace_type ON traces
FOR EACH ROW EXECUTE FUNCTION validate_trace_continuation();

-- Link changes and derived thread bookkeeping share the trace write transaction.
-- This trigger runs before refresh_writing_threads_from_trace (alphabetical order).
CREATE FUNCTION attach_writing_thread_from_link() RETURNS TRIGGER LANGUAGE plpgsql AS $$
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
CREATE TRIGGER attach_writing_thread_from_link AFTER INSERT OR UPDATE OF continues_trace_id ON traces
FOR EACH ROW EXECUTE FUNCTION attach_writing_thread_from_link();

-- Invalidation lives at the record boundary, so PATCH, timeout finalization,
-- archival and background writes all invalidate projections consistently.
CREATE FUNCTION refresh_writing_threads_from_trace() RETURNS TRIGGER LANGUAGE plpgsql AS $$
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
        input_revision=input_revision+1,description=NULL,search_tags='{}',search_vector=''::tsvector,
        compiled_revision=NULL,generation_status='PENDING',generation_due_at=NOW()+INTERVAL '15 seconds',
        generation_claim_id=NULL,generation_lease_until=NULL,generation_error=NULL,updated_at=NOW()
    WHERE w.user_id=NEW.user_id AND EXISTS(SELECT 1 FROM writing_thread_trace_ids(w.id) c WHERE c.trace_id=NEW.id);
    RETURN NEW;
END $$;
CREATE TRIGGER refresh_writing_threads_from_trace AFTER UPDATE OF title,subtitle,content,status,is_encrypted,encryption_metadata,finalized_at ON traces
FOR EACH ROW WHEN (OLD.title IS DISTINCT FROM NEW.title OR OLD.subtitle IS DISTINCT FROM NEW.subtitle
    OR OLD.content IS DISTINCT FROM NEW.content OR OLD.status IS DISTINCT FROM NEW.status
    OR OLD.is_encrypted IS DISTINCT FROM NEW.is_encrypted OR OLD.encryption_metadata IS DISTINCT FROM NEW.encryption_metadata
    OR OLD.finalized_at IS DISTINCT FROM NEW.finalized_at)
EXECUTE FUNCTION refresh_writing_threads_from_trace();

ALTER TABLE usage_events DROP CONSTRAINT usage_events_event_type_check;
ALTER TABLE usage_events ADD CONSTRAINT usage_events_event_type_check CHECK (event_type IN (
    'ACCOUNT_CREATED','ACCOUNT_DELETED','ACCOUNT_SELF_DELETED','ACCOUNT_ADMIN_DELETED','HOME_VISITED','HISTORY_VISITED',
    'FEED_VISITED','FEED_ENGAGED_30S','SHARED_JOURNAL_LANDING_VISITED','SHARED_JOURNAL_SIGNUP_CLICKED','JOURNAL_OPENED',
    'FOLLOWED_JOURNAL_OPENED','POST_OPENED','FEEDBACK_OPENED','SUMMARY_OPENED','TRACE_TIMEOUT_SET','TRACE_TIMEOUT_EXTENDED',
    'TRACE_TIMEOUT_AUTO_FINALIZED','AI_TRANSCRIPTION_REQUESTED','AI_WAL_COMPILATION_REQUESTED','AI_WRITING_THREAD_COMPILATION_REQUESTED'
));
