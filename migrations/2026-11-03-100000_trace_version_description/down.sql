ALTER TABLE trace_versions DROP COLUMN description;

CREATE OR REPLACE FUNCTION immutable_trace_version() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF (to_jsonb(NEW)-'content_image_asset_id') IS DISTINCT FROM (to_jsonb(OLD)-'content_image_asset_id')
        OR (NEW.content_image_asset_id IS DISTINCT FROM OLD.content_image_asset_id AND NEW.content_image_asset_id IS NOT NULL) THEN
        RAISE EXCEPTION 'Archived trace versions cannot be edited' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
