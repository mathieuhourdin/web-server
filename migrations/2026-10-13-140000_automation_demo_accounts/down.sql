-- Demo users must be purged first: dropping the marker must not turn them into normal users.
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM automation_demo_users) THEN
        RAISE EXCEPTION 'Purge demo accounts before reverting automation persistence';
    END IF;
END $$;
DROP TRIGGER automation_demo_email ON outbound_emails;
DROP TRIGGER automation_demo_role ON user_roles;
DROP TRIGGER automation_user_identity ON users;
DROP TRIGGER automation_demo_identity ON automation_demo_users;
DROP TRIGGER automation_public_link_boundary ON journal_share_links;
DROP TRIGGER automation_grant_boundary ON post_grants;
DROP TRIGGER automation_relationship_boundary ON relationships;
DROP FUNCTION automation_suppress_email();
DROP FUNCTION automation_guard_role();
DROP FUNCTION automation_guard_demo_identity();
DROP FUNCTION automation_guard_public_link();
DROP FUNCTION automation_guard_grant();
DROP FUNCTION automation_guard_relationship();
CREATE OR REPLACE FUNCTION trace_publication_readable(publication UUID,viewer UUID) RETURNS BOOLEAN
LANGUAGE SQL STABLE AS $$
    SELECT trace_publication_eligible(publication,viewer) AND EXISTS (
        SELECT 1 FROM posts p WHERE p.id=publication AND (p.user_id=viewer OR EXISTS (
            SELECT 1 FROM post_grants g WHERE g.post_id=p.id AND g.status='ACTIVE'
            AND (g.grantee_user_id=viewer OR (g.grantee_scope='ALL_PLATFORM_USERS'
                AND EXISTS(SELECT 1 FROM users u WHERE u.id=viewer AND u.is_platform_user AND u.principal_type='HUMAN')))
        ))
    )
$$;
DROP FUNCTION automation_same_sandbox(UUID,UUID);
DROP FUNCTION automation_user_discoverable(UUID,UUID);
DELETE FROM outbound_emails WHERE status='SUPPRESSED';
ALTER TABLE outbound_emails DROP CONSTRAINT outbound_emails_status_check;
ALTER TABLE outbound_emails ADD CONSTRAINT outbound_emails_status_check CHECK (status IN ('PENDING','RUNNING','SENT','FAILED'));
DROP TABLE automation_ai_requests,automation_audit_events,automation_delegations,automation_demo_users,automation_clients;
