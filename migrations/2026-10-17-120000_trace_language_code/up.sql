ALTER TABLE traces ADD COLUMN language_code TEXT;
ALTER TABLE traces ADD CONSTRAINT traces_language_code_check
    CHECK (language_code IS NULL OR language_code ~ '^[a-z]{2,3}$');

-- A language backfill is metadata maintenance, not a user edit. Run after
-- Diesel's timestamp trigger, preserving the edit date for language-only writes.
CREATE FUNCTION preserve_trace_language_timestamp() RETURNS trigger AS $$
BEGIN
    IF (to_jsonb(NEW) - 'language_code' - 'updated_at')
       IS NOT DISTINCT FROM (to_jsonb(OLD) - 'language_code' - 'updated_at') THEN
        NEW.updated_at := OLD.updated_at;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER zz_trace_language_timestamp
BEFORE UPDATE OF language_code ON traces
FOR EACH ROW EXECUTE FUNCTION preserve_trace_language_timestamp();
