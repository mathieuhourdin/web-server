-- Disposable database only, with migrations through 20261010160000 applied.
BEGIN;
DO $$ BEGIN
    IF current_database()<>'audience_test' THEN
        RAISE EXCEPTION 'Only run this check in audience_test';
    END IF;
END $$;
CREATE TEMP TABLE before_source_migration AS SELECT * FROM post_grants;
\ir ../../migrations/2026-10-10-170000_multiple_post_grant_sources/up.sql
DO $$ BEGIN
    IF EXISTS ((SELECT * FROM before_source_migration EXCEPT SELECT * FROM post_grants)
        UNION ALL (SELECT * FROM post_grants EXCEPT SELECT * FROM before_source_migration))
    THEN RAISE EXCEPTION 'Source migration altered existing grants'; END IF;
    IF EXISTS (
        SELECT 1 FROM post_grants g LEFT JOIN post_grant_sources s ON s.post_grant_id=g.id
        WHERE g.source_grant_id IS NULL
        GROUP BY g.id,g.status,g.audience_id HAVING count(s.id)<>1
            OR bool_or(s.status<>g.status OR s.audience_id IS DISTINCT FROM g.audience_id)
    ) THEN RAISE EXCEPTION 'Source migration did not preserve the sole existing origin'; END IF;
    IF EXISTS (SELECT 1 FROM post_grants g JOIN post_grant_sources s ON s.post_grant_id=g.id
        WHERE g.source_grant_id IS NOT NULL)
    THEN RAISE EXCEPTION 'Inherited grants must continue following their parent'; END IF;
END $$;
COMMIT;
