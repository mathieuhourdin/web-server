DROP INDEX outbound_email_distribution_once_idx;
UPDATE outbound_emails SET resource_type='POST_PUBLICATION' WHERE resource_type='POST_DISTRIBUTION_EVENT';
CREATE UNIQUE INDEX outbound_email_publication_once_idx ON outbound_emails(resource_id,recipient_user_id,reason)
    WHERE resource_type='POST_PUBLICATION';

ALTER TABLE post_distribution_events RENAME TO post_publications;
ALTER TABLE post_publications RENAME COLUMN distributed_at TO published_at;
ALTER TABLE post_distribution_recipients RENAME TO post_publication_recipients;
ALTER TABLE post_publication_recipients RENAME COLUMN distribution_event_id TO publication_id;
ALTER TABLE posts RENAME COLUMN last_distribution_event_id TO last_publication_id;
ALTER TABLE posts RENAME COLUMN last_distribution_at TO last_publication_at;

ALTER TABLE post_publications RENAME CONSTRAINT post_distribution_events_pkey TO post_publications_pkey;
ALTER TABLE post_publications RENAME CONSTRAINT post_distribution_events_post_id_fkey TO post_publications_post_id_fkey;
ALTER TABLE post_publications RENAME CONSTRAINT post_distribution_events_trace_version_id_fkey TO post_publications_trace_version_id_fkey;
ALTER TABLE post_publications RENAME CONSTRAINT post_distribution_events_notification_mode_check TO post_publications_notification_mode_check;
ALTER TABLE post_publications RENAME CONSTRAINT post_distribution_events_post_id_idempotency_key_key TO post_publications_post_id_idempotency_key_key;
ALTER TABLE post_publication_recipients RENAME CONSTRAINT post_distribution_recipients_pkey TO post_publication_recipients_pkey;
ALTER TABLE post_publication_recipients RENAME CONSTRAINT post_distribution_recipients_distribution_event_id_fkey TO post_publication_recipients_publication_id_fkey;
ALTER TABLE post_publication_recipients RENAME CONSTRAINT post_distribution_recipients_user_id_fkey TO post_publication_recipients_user_id_fkey;
ALTER TABLE post_publication_recipients RENAME CONSTRAINT post_distribution_recipients_email_id_fkey TO post_publication_recipients_email_id_fkey;
ALTER TABLE post_publication_recipients RENAME CONSTRAINT post_distribution_recipients_push_status_check TO post_publication_recipients_push_status_check;
ALTER TABLE posts RENAME CONSTRAINT posts_last_distribution_event_id_fkey TO posts_last_publication_id_fkey;
ALTER INDEX post_distribution_events_history_idx RENAME TO post_publications_history_idx;
ALTER INDEX posts_last_distribution_idx RENAME TO posts_last_publication_idx;

DO $$ DECLARE f RECORD; definition TEXT; BEGIN
    FOR f IN SELECT p.oid FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
        WHERE n.nspname=current_schema() AND p.proname IN (
            'validate_post_distribution_event','materialize_post_distribution_event',
            'record_post_distribution_transition','immutable_post_distribution_event','delete_post_distribution_emails')
    LOOP
        definition := pg_get_functiondef(f.oid);
        definition := replace(definition,'post_distribution_recipients','post_publication_recipients');
        definition := replace(definition,'post_distribution_events','post_publications');
        definition := replace(definition,'last_distribution_event_id','last_publication_id');
        definition := replace(definition,'last_distribution_at','last_publication_at');
        definition := replace(definition,'distribution_event_id','publication_id');
        definition := replace(definition,'distributed_at','published_at');
        definition := replace(definition,'POST_DISTRIBUTION_EVENT','POST_PUBLICATION');
        definition := replace(definition,'distribution event','publication event');
        definition := replace(definition,'Distribution event history','Publication history');
        EXECUTE definition;
    END LOOP;
END $$;

ALTER FUNCTION validate_post_distribution_event() RENAME TO validate_post_publication_event;
ALTER FUNCTION materialize_post_distribution_event() RENAME TO materialize_post_publication_event;
ALTER FUNCTION record_post_distribution_transition() RENAME TO record_post_publish_transition;
ALTER FUNCTION immutable_post_distribution_event() RENAME TO immutable_post_publication;
ALTER FUNCTION delete_post_distribution_emails() RENAME TO delete_publication_emails;
ALTER TRIGGER validate_post_distribution_event ON post_publications RENAME TO validate_post_publication_event;
ALTER TRIGGER materialize_post_distribution_event ON post_publications RENAME TO materialize_post_publication_event;
ALTER TRIGGER immutable_post_distribution_event ON post_publications RENAME TO immutable_post_publication;
ALTER TRIGGER delete_post_distribution_emails ON post_publications RENAME TO delete_publication_emails;
ALTER TRIGGER zz_record_post_distribution_transition ON posts RENAME TO zz_record_post_publish_transition;
