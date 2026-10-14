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
        input_revision=input_revision+1,description=NULL,search_tags='{}',search_vector=''::tsvector,
        compiled_revision=NULL,generation_status='PENDING',generation_due_at=NOW()+INTERVAL '15 seconds',
        generation_claim_id=NULL,generation_lease_until=NULL,generation_error=NULL,updated_at=NOW()
    WHERE w.user_id=NEW.user_id AND EXISTS(SELECT 1 FROM writing_thread_trace_ids(w.id) c WHERE c.trace_id=NEW.id);
    RETURN NEW;
END $$;

UPDATE writing_threads SET description=COALESCE(journey_summary,description),
    input_revision=input_revision+1,compiled_revision=NULL,search_vector=''::tsvector,
    generation_status='PENDING',generation_due_at=NOW()+INTERVAL '15 seconds',
    generation_claim_id=NULL,generation_lease_until=NULL,generation_error=NULL;
ALTER TABLE writing_threads DROP COLUMN title;
ALTER TABLE writing_threads DROP COLUMN ongoing_question;
ALTER TABLE writing_threads DROP COLUMN journey_summary;
