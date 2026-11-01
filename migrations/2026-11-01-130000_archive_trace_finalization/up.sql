-- Archive exactly the committed finalization content, in the same transaction.
-- Existing finalized traces are not backfilled, and later edits are not versions.
-- Match the existing archival API: encrypted traces are not plaintext snapshots.
CREATE FUNCTION archive_finalized_trace_version() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='INSERT' OR OLD.status IS DISTINCT FROM 'FINALIZED' THEN
        INSERT INTO trace_versions(trace_id,source_version,title,subtitle,content,
            content_image_asset_id,language_code,finalized_at)
        VALUES(NEW.id,NEW.version_integer,NEW.title,NEW.subtitle,NEW.content,
            NEW.content_image_asset_id,NEW.language_code,NEW.finalized_at);
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER archive_finalized_trace_version AFTER INSERT OR UPDATE OF status ON traces
    FOR EACH ROW WHEN(NEW.status='FINALIZED' AND NOT NEW.is_encrypted)
    EXECUTE FUNCTION archive_finalized_trace_version();
