-- Data-preserving terminology change. No event inserts, recipient rematerialization,
-- timestamp changes or dispatch resets: this migration must not notify anyone.
ALTER TABLE post_publications RENAME TO post_distribution_events;
ALTER TABLE post_distribution_events RENAME COLUMN published_at TO distributed_at;
ALTER TABLE post_publication_recipients RENAME TO post_distribution_recipients;
ALTER TABLE post_distribution_recipients RENAME COLUMN publication_id TO distribution_event_id;
ALTER TABLE posts RENAME COLUMN last_publication_id TO last_distribution_event_id;
ALTER TABLE posts RENAME COLUMN last_publication_at TO last_distribution_at;

ALTER TABLE post_distribution_events RENAME CONSTRAINT post_publications_pkey TO post_distribution_events_pkey;
ALTER TABLE post_distribution_events RENAME CONSTRAINT post_publications_post_id_fkey TO post_distribution_events_post_id_fkey;
ALTER TABLE post_distribution_events RENAME CONSTRAINT post_publications_trace_version_id_fkey TO post_distribution_events_trace_version_id_fkey;
ALTER TABLE post_distribution_events RENAME CONSTRAINT post_publications_notification_mode_check TO post_distribution_events_notification_mode_check;
ALTER TABLE post_distribution_events RENAME CONSTRAINT post_publications_post_id_idempotency_key_key TO post_distribution_events_post_id_idempotency_key_key;
ALTER TABLE post_distribution_recipients RENAME CONSTRAINT post_publication_recipients_pkey TO post_distribution_recipients_pkey;
ALTER TABLE post_distribution_recipients RENAME CONSTRAINT post_publication_recipients_publication_id_fkey TO post_distribution_recipients_distribution_event_id_fkey;
ALTER TABLE post_distribution_recipients RENAME CONSTRAINT post_publication_recipients_user_id_fkey TO post_distribution_recipients_user_id_fkey;
ALTER TABLE post_distribution_recipients RENAME CONSTRAINT post_publication_recipients_email_id_fkey TO post_distribution_recipients_email_id_fkey;
ALTER TABLE post_distribution_recipients RENAME CONSTRAINT post_publication_recipients_push_status_check TO post_distribution_recipients_push_status_check;
ALTER TABLE posts RENAME CONSTRAINT posts_last_publication_id_fkey TO posts_last_distribution_event_id_fkey;
ALTER INDEX post_publications_history_idx RENAME TO post_distribution_events_history_idx;
ALTER INDEX posts_last_publication_idx RENAME TO posts_last_distribution_idx;

-- PL/pgSQL bodies retain textual identifiers across ALTER TABLE RENAME. Rewrite
-- just the five existing event functions, preserving their authorization,
-- immutability and once-only materialization/dispatch behavior.
DO $$ DECLARE f RECORD; definition TEXT; BEGIN
    FOR f IN SELECT p.oid FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
        WHERE n.nspname=current_schema() AND p.proname IN (
            'validate_post_publication_event','materialize_post_publication_event',
            'record_post_publish_transition','immutable_post_publication','delete_publication_emails')
    LOOP
        definition := pg_get_functiondef(f.oid);
        definition := replace(definition,'post_publication_recipients','post_distribution_recipients');
        definition := replace(definition,'post_publications','post_distribution_events');
        definition := replace(definition,'last_publication_id','last_distribution_event_id');
        definition := replace(definition,'last_publication_at','last_distribution_at');
        definition := replace(definition,'publication_id','distribution_event_id');
        definition := replace(definition,'published_at','distributed_at');
        definition := replace(definition,'POST_PUBLICATION','POST_DISTRIBUTION_EVENT');
        definition := replace(definition,'publication event','distribution event');
        definition := replace(definition,'Publication history','Distribution event history');
        EXECUTE definition;
    END LOOP;
END $$;

ALTER FUNCTION validate_post_publication_event() RENAME TO validate_post_distribution_event;
ALTER FUNCTION materialize_post_publication_event() RENAME TO materialize_post_distribution_event;
ALTER FUNCTION record_post_publish_transition() RENAME TO record_post_distribution_transition;
ALTER FUNCTION immutable_post_publication() RENAME TO immutable_post_distribution_event;
ALTER FUNCTION delete_publication_emails() RENAME TO delete_post_distribution_emails;
ALTER TRIGGER validate_post_publication_event ON post_distribution_events RENAME TO validate_post_distribution_event;
ALTER TRIGGER materialize_post_publication_event ON post_distribution_events RENAME TO materialize_post_distribution_event;
ALTER TRIGGER immutable_post_publication ON post_distribution_events RENAME TO immutable_post_distribution_event;
ALTER TRIGGER delete_publication_emails ON post_distribution_events RENAME TO delete_post_distribution_emails;
ALTER TRIGGER zz_record_post_publish_transition ON posts RENAME TO zz_record_post_distribution_transition;

-- Preserve queued/sent emails and their recipient links; only their resource tag
-- changes. Build the once-only index with the new tag in the same transaction.
DROP INDEX outbound_email_publication_once_idx;
UPDATE outbound_emails SET resource_type='POST_DISTRIBUTION_EVENT' WHERE resource_type='POST_PUBLICATION';
CREATE UNIQUE INDEX outbound_email_distribution_once_idx ON outbound_emails(resource_id,recipient_user_id,reason)
    WHERE resource_type='POST_DISTRIBUTION_EVENT';
