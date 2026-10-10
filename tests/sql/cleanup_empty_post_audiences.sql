-- Run with psql -v ON_ERROR_STOP=1 -f tests/sql/cleanup_empty_post_audiences.sql
-- against a disposable clone with migrations through 20261010150000 applied.
BEGIN;
DO $$ BEGIN
    IF current_database() <> 'audience_cleanup_test' THEN
        RAISE EXCEPTION 'Only run this fixture in audience_cleanup_test';
    END IF;
END $$;

CREATE TEMP TABLE cleanup_users AS
SELECT uuid_generate_v4() AS owner, uuid_generate_v4() AS reader,
       uuid_generate_v4() AS other, uuid_generate_v4() AS journal;
INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
SELECT id, id::text || '@test.invalid', 'Cleanup', 'Test', '@' || id::text, 'unused', NOW()
FROM cleanup_users u CROSS JOIN LATERAL (VALUES(u.owner),(u.reader),(u.other)) ids(id);
INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
SELECT journal, owner, 'Cleanup test', '', '', 'USER_JOURNAL', 'ACTIVE' FROM cleanup_users;

CREATE TEMP TABLE cleanup_cases AS
SELECT kind, uuid_generate_v4() AS trace_id, uuid_generate_v4() AS post_id,
       uuid_generate_v4() AS audience_id
FROM (VALUES('empty'),('active'),('revoked'),('mixed')) cases(kind);
INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at)
SELECT trace_id, owner, journal, 'Cleanup test', '', '', NOW(), 'USER_TRACE', 'FINALIZED', NOW(), NOW()
FROM cleanup_cases CROSS JOIN cleanup_users;
INSERT INTO posts(id,user_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,public_link_eligible,publishing_date)
SELECT post_id, owner, trace_id, '', '', '', 'OUTPUT', 'IDEA', 'PUBLISHED', TRUE, NOW()
FROM cleanup_cases CROSS JOIN cleanup_users;
INSERT INTO audiences(id,owner_user_id,name)
SELECT audience_id, owner, 'Cleanup ' || kind FROM cleanup_cases CROSS JOIN cleanup_users;
INSERT INTO post_audiences(post_id,audience_id,is_default)
SELECT post_id, audience_id, kind='revoked' FROM cleanup_cases;
INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,access_level,status,audience_id)
SELECT uuid_generate_v4(), post_id, owner, reader, 'READ',
       CASE WHEN kind='active' THEN 'ACTIVE' ELSE 'REVOKED' END, audience_id
FROM cleanup_cases CROSS JOIN cleanup_users WHERE kind<>'empty';
INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,access_level,status,audience_id)
SELECT uuid_generate_v4(), post_id, owner, other, 'READ', 'ACTIVE', audience_id
FROM cleanup_cases CROSS JOIN cleanup_users WHERE kind='mixed';

CREATE TEMP TABLE before_associations AS SELECT * FROM post_audiences;
CREATE TEMP TABLE before_grants AS SELECT * FROM post_grants;

\ir ../../migrations/2026-10-10-160000_cleanup_empty_post_audiences/up.sql

DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM cleanup_cases c
        WHERE (EXISTS (SELECT 1 FROM post_audiences pa WHERE pa.post_id=c.post_id))
            <> (c.kind<>'revoked')
    ) THEN RAISE EXCEPTION 'Cleanup did not preserve empty/active/mixed associations'; END IF;
    IF NOT EXISTS (
        SELECT 1 FROM removed_post_audiences_20261010 b JOIN cleanup_cases c USING(post_id,audience_id)
        WHERE c.kind='revoked' AND b.is_default
    ) THEN RAISE EXCEPTION 'Removed association was not backed up'; END IF;
    IF EXISTS ((SELECT * FROM before_grants EXCEPT SELECT * FROM post_grants)
        UNION ALL (SELECT * FROM post_grants EXCEPT SELECT * FROM before_grants))
    THEN RAISE EXCEPTION 'Cleanup changed grants'; END IF;
END $$;

\ir ../../migrations/2026-10-10-160000_cleanup_empty_post_audiences/down.sql

DO $$ BEGIN
    IF EXISTS ((SELECT * FROM before_associations EXCEPT SELECT * FROM post_audiences)
        UNION ALL (SELECT * FROM post_audiences EXCEPT SELECT * FROM before_associations))
    THEN RAISE EXCEPTION 'Rollback did not exactly restore associations'; END IF;
END $$;
ROLLBACK;
