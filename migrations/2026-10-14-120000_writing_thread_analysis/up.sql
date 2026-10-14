-- Keep the legacy description column only for rollback; it is no longer exposed.
ALTER TABLE writing_threads ADD COLUMN title TEXT;
ALTER TABLE writing_threads ADD COLUMN ongoing_question TEXT;
ALTER TABLE writing_threads ADD COLUMN journey_summary TEXT;

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

-- The new analysis cannot be inferred from the old recap. Regenerate from source.
-- Incrementing revisions and releasing leases fences any old-format in-flight call.
UPDATE writing_threads SET input_revision=input_revision+1,
    description=NULL,title=NULL,ongoing_question=NULL,journey_summary=NULL,
    search_tags='{}',search_vector=''::tsvector,compiled_revision=NULL,
    generation_status='PENDING',generation_due_at=NOW()+INTERVAL '15 seconds',
    generation_claim_id=NULL,generation_lease_until=NULL,generation_error=NULL,generated_at=NULL;
