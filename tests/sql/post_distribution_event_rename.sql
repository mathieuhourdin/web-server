-- Run with psql --single-transaction -v ON_ERROR_STOP=1 on a disposable blog_test
-- database migrated up to (but not including) the distribution-event rename.
DO $$ BEGIN
    IF current_database()<>'blog_test' OR to_regclass('post_publications') IS NULL THEN
        RAISE EXCEPTION 'Requires disposable blog_test with the pre-rename schema';
    END IF;
END $$;

CREATE TEMP TABLE rename_ids AS SELECT gen_random_uuid() AS owner_id,
    gen_random_uuid() AS reader_id,gen_random_uuid() AS journal_id,
    gen_random_uuid() AS trace_id,gen_random_uuid() AS post_id,
    gen_random_uuid() AS version_id,gen_random_uuid() AS event_id,gen_random_uuid() AS email_id;
INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
SELECT owner_id,owner_id||'@test.invalid','Owner','Test',owner_id::text,'unused',NOW() FROM rename_ids
UNION ALL SELECT reader_id,reader_id||'@test.invalid','Reader','Test',reader_id::text,'unused',NOW() FROM rename_ids;
INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
SELECT journal_id,owner_id,'Test','','','USER_JOURNAL','ACTIVE' FROM rename_ids;
INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at)
SELECT trace_id,owner_id,journal_id,'Synthetic','','Synthetic',NOW(),'USER_TRACE','FINALIZED',NOW(),NOW() FROM rename_ids;
INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
SELECT reader_id,owner_id,'FOLLOW','ACCEPTED',NOW() FROM rename_ids;
INSERT INTO posts(id,user_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,notification_mode)
SELECT post_id,owner_id,trace_id,'','','','OUTPUT','IDEA','DRAFT','NONE' FROM rename_ids;
INSERT INTO post_grants(id,post_id,owner_user_id,grantee_user_id,access_level,status)
SELECT gen_random_uuid(),post_id,owner_id,reader_id,'READ','ACTIVE' FROM rename_ids;
UPDATE posts SET status='PUBLISHED' WHERE id=(SELECT post_id FROM rename_ids);
INSERT INTO trace_versions(id,trace_id,source_version,title,subtitle,content)
SELECT version_id,trace_id,1,'Synthetic','','Synthetic' FROM rename_ids;
INSERT INTO post_publications(id,post_id,trace_version_id,notification_mode,notification_user_ids)
SELECT event_id,post_id,version_id,'SELECTED',ARRAY[reader_id] FROM rename_ids;
UPDATE post_publications SET dispatch_started_at='2026-01-01' WHERE id=(SELECT event_id FROM rename_ids);
INSERT INTO outbound_emails(id,recipient_user_id,reason,resource_type,resource_id,to_email,from_email,subject,status,provider,scheduled_at)
SELECT email_id,reader_id,'POST_PUBLISHED','POST_PUBLICATION',event_id,
    'reader@test.invalid','owner@test.invalid','Synthetic','PENDING','RESEND','2099-01-01'::timestamp FROM rename_ids;
UPDATE post_publication_recipients SET push_status='SENT',push_sent_at='2026-01-02',
    email_id=(SELECT email_id FROM rename_ids) WHERE publication_id=(SELECT event_id FROM rename_ids);

CREATE TEMP TABLE rename_before AS SELECT
    (SELECT jsonb_agg((to_jsonb(e)-'published_at')||jsonb_build_object('distributed_at',e.published_at) ORDER BY e.id)
        FROM post_publications e) AS events,
    (SELECT jsonb_agg((to_jsonb(r)-'publication_id')||jsonb_build_object('distribution_event_id',r.publication_id)
        ORDER BY r.publication_id,r.user_id) FROM post_publication_recipients r) AS recipients,
    (SELECT jsonb_agg(jsonb_build_object('id',p.id,'last_distribution_event_id',p.last_publication_id,
        'last_distribution_at',p.last_publication_at) ORDER BY p.id) FROM posts p) AS posts,
    (SELECT jsonb_agg((to_jsonb(e)-'resource_type'-'updated_at')||jsonb_build_object('resource_type',
        CASE WHEN e.resource_type='POST_PUBLICATION' THEN 'POST_DISTRIBUTION_EVENT' ELSE e.resource_type END)
        ORDER BY e.id) FROM outbound_emails e) AS emails;

\ir ../../migrations/2026-11-01-120000_post_distribution_events/up.sql

CREATE FUNCTION pg_temp.assert_distribution_rename() RETURNS void LANGUAGE plpgsql AS $$
DECLARE expected rename_before%ROWTYPE; BEGIN
    SELECT * INTO expected FROM rename_before;
    IF to_regclass('post_publications') IS NOT NULL OR to_regclass('post_publication_recipients') IS NOT NULL THEN
        RAISE EXCEPTION 'Old relations must not remain as aliases';
    END IF;
    IF expected.events IS DISTINCT FROM (SELECT jsonb_agg(to_jsonb(e) ORDER BY e.id) FROM post_distribution_events e) THEN
        RAISE EXCEPTION 'Event data changed or an event was replayed';
    END IF;
    IF expected.recipients IS DISTINCT FROM (SELECT jsonb_agg(to_jsonb(r) ORDER BY r.distribution_event_id,r.user_id)
        FROM post_distribution_recipients r) THEN
        RAISE EXCEPTION 'Recipient history/delivery state changed';
    END IF;
    IF expected.posts IS DISTINCT FROM (SELECT jsonb_agg(jsonb_build_object('id',p.id,
        'last_distribution_event_id',p.last_distribution_event_id,'last_distribution_at',p.last_distribution_at)
        ORDER BY p.id) FROM posts p) THEN
        RAISE EXCEPTION 'Feed ordering pointers/timestamps changed';
    END IF;
    IF expected.emails IS DISTINCT FROM (SELECT jsonb_agg(to_jsonb(e)-'updated_at' ORDER BY e.id) FROM outbound_emails e) THEN
        RAISE EXCEPTION 'Email history changed beyond the resource tag';
    END IF;
END $$;
SELECT pg_temp.assert_distribution_rename();

-- Exercise the reverse migration and its original transition/materialization
-- triggers, then restore the renamed schema for the existing Rust scenarios.
\ir ../../migrations/2026-11-01-120000_post_distribution_events/down.sql
SAVEPOINT old_trigger_probe;
INSERT INTO post_publications(post_id,notification_mode) SELECT post_id,'NONE' FROM rename_ids;
ROLLBACK TO old_trigger_probe;
RELEASE old_trigger_probe;
\ir ../../migrations/2026-11-01-120000_post_distribution_events/up.sql
SELECT pg_temp.assert_distribution_rename();

-- Remove the synthetic posts before their author, avoiding unrelated source-
-- archival triggers during owner deletion and exercising event-email cleanup.
DELETE FROM posts WHERE user_id=(SELECT owner_id FROM rename_ids);
DELETE FROM users WHERE id IN(SELECT owner_id FROM rename_ids UNION ALL SELECT reader_id FROM rename_ids);
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM outbound_emails WHERE id=(SELECT email_id FROM rename_ids)) THEN
        RAISE EXCEPTION 'Event deletion did not clean up its linked email';
    END IF;
END $$;
