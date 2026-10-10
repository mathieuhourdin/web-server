-- Multiple sharing decisions cannot be faithfully collapsed to one origin.
-- Use a pre-migration backup for a coordinated rollback, rather than silently
-- altering who loses access on a future audience removal.
DO $$ BEGIN
    RAISE EXCEPTION 'Multiple grant sources require restoring a pre-migration backup to roll back safely';
END $$;
