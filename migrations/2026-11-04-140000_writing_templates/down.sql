-- Do not silently discard trace provenance.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM traces WHERE writing_template_id IS NOT NULL) THEN
        RAISE EXCEPTION 'Cannot remove writing templates while traces reference them';
    END IF;
END $$;
ALTER TABLE traces DROP COLUMN writing_template_id;
DROP TABLE writing_templates;
