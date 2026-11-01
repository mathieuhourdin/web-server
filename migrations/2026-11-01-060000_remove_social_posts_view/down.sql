CREATE VIEW social_posts AS SELECT * FROM posts WHERE publication_channel_id IS NULL
    WITH CASCADED CHECK OPTION;
-- Restore the compatibility relation for older binaries without changing data.
DO $$ DECLARE f RECORD; definition TEXT; BEGIN
    FOR f IN SELECT p.oid FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
        WHERE n.nspname=current_schema() AND p.proname IN
        ('trace_publication_eligible','trace_publication_readable',
         'validate_trace_publication','sync_complement_grants')
    LOOP
        -- The view supplies the predicate again. Remove the explicit predicates
        -- too, so older channel migrations can subsequently roll back cleanly.
        definition := regexp_replace(pg_get_functiondef(f.oid),
            '(p[.]|audience[.])?publication_channel_id IS NULL[[:space:]]+AND[[:space:]]+', '', 'g');
        EXECUTE replace(replace(definition,
            'FROM posts ', 'FROM social_posts '), 'JOIN posts ', 'JOIN social_posts ');
    END LOOP;
END $$;
