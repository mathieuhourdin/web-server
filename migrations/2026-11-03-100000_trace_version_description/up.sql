ALTER TABLE trace_versions
    ADD COLUMN description TEXT,
    ADD CONSTRAINT trace_versions_description_length_check
        CHECK (description IS NULL OR char_length(description) <= 2000);

CREATE OR REPLACE FUNCTION immutable_trace_version() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    -- The owner's annotation is editable; the archived snapshot remains frozen.
    -- Preserve FK-driven clearing when an image is permanently deleted.
    IF (to_jsonb(NEW)-'content_image_asset_id'-'description')
        IS DISTINCT FROM (to_jsonb(OLD)-'content_image_asset_id'-'description')
        OR (NEW.content_image_asset_id IS DISTINCT FROM OLD.content_image_asset_id
            AND NEW.content_image_asset_id IS NOT NULL) THEN
        RAISE EXCEPTION 'Archived trace versions cannot be edited' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
