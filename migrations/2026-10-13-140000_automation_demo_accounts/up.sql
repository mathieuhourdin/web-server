CREATE TABLE automation_clients (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 100),
    scopes TEXT[] NOT NULL,
    token_hash TEXT NOT NULL,
    max_demo_users INTEGER NOT NULL DEFAULT 100 CHECK (max_demo_users BETWEEN 1 AND 1000),
    ai_daily_limit INTEGER NOT NULL DEFAULT 100 CHECK (ai_daily_limit BETWEEN 0 AND 1000),
    revoked_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE TABLE automation_demo_users (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    api_client_id UUID NOT NULL REFERENCES automation_clients(id),
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE INDEX automation_demo_users_client_idx ON automation_demo_users(api_client_id,created_at,user_id);
CREATE TABLE automation_delegations (
    session_id UUID PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    api_client_id UUID NOT NULL REFERENCES automation_clients(id),
    user_id UUID NOT NULL REFERENCES automation_demo_users(user_id) ON DELETE CASCADE,
    scopes TEXT[] NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE TABLE automation_audit_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    api_client_id UUID NOT NULL REFERENCES automation_clients(id),
    acting_user_id UUID,
    action TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE INDEX automation_audit_client_idx ON automation_audit_events(api_client_id,created_at);
CREATE TABLE automation_ai_requests (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    api_client_id UUID NOT NULL REFERENCES automation_clients(id),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    request_kind TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
CREATE INDEX automation_ai_requests_budget_idx ON automation_ai_requests(api_client_id,created_at);

CREATE FUNCTION automation_user_discoverable(target UUID,viewer UUID) RETURNS BOOLEAN
LANGUAGE SQL STABLE AS $$
    SELECT NOT EXISTS(SELECT 1 FROM automation_demo_users WHERE user_id=target)
        OR EXISTS(SELECT 1 FROM automation_demo_users a JOIN automation_demo_users b
            ON a.api_client_id=b.api_client_id WHERE a.user_id=target AND b.user_id=viewer)
$$;

CREATE FUNCTION automation_same_sandbox(a UUID,b UUID) RETURNS BOOLEAN
LANGUAGE SQL STABLE AS $$
    SELECT COALESCE((SELECT api_client_id FROM automation_demo_users WHERE user_id=a)
        = (SELECT api_client_id FROM automation_demo_users WHERE user_id=b),
        NOT EXISTS(SELECT 1 FROM automation_demo_users WHERE user_id IN (a,b)))
$$;

-- Keep all publication reads inside the sandbox too, including ALL_PLATFORM_USERS
-- grants on real accounts. This also prevents demos leaving seen-state footprints
-- on real users' otherwise-public posts.
CREATE OR REPLACE FUNCTION trace_publication_readable(publication UUID,viewer UUID) RETURNS BOOLEAN
LANGUAGE SQL STABLE AS $$
    SELECT trace_publication_eligible(publication,viewer) AND EXISTS (
        SELECT 1 FROM posts p WHERE p.id=publication AND automation_same_sandbox(p.user_id,viewer)
        AND (p.user_id=viewer OR EXISTS (
            SELECT 1 FROM post_grants g WHERE g.post_id=p.id AND g.status='ACTIVE'
            AND (g.grantee_user_id=viewer OR (g.grantee_scope='ALL_PLATFORM_USERS'
                AND EXISTS(SELECT 1 FROM users u WHERE u.id=viewer AND u.is_platform_user AND u.principal_type='HUMAN')))
        ))
    )
$$;

CREATE FUNCTION automation_guard_relationship() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NOT automation_same_sandbox(NEW.requester_user_id,NEW.target_user_id) THEN
        RAISE EXCEPTION 'Demo relationships must stay within the same agent sandbox' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER automation_relationship_boundary BEFORE INSERT OR UPDATE ON relationships
FOR EACH ROW EXECUTE FUNCTION automation_guard_relationship();

CREATE FUNCTION automation_guard_grant() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.grantee_user_id IS NOT NULL AND NOT automation_same_sandbox(NEW.owner_user_id,NEW.grantee_user_id))
       OR (NEW.grantee_scope IS NOT NULL AND EXISTS(SELECT 1 FROM automation_demo_users WHERE user_id=NEW.owner_user_id)) THEN
        RAISE EXCEPTION 'Demo grants must stay within the same agent sandbox' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER automation_grant_boundary BEFORE INSERT OR UPDATE ON post_grants
FOR EACH ROW EXECUTE FUNCTION automation_guard_grant();

CREATE FUNCTION automation_guard_public_link() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS(SELECT 1 FROM automation_demo_users WHERE user_id=NEW.owner_user_id) THEN
        RAISE EXCEPTION 'Demo accounts cannot create public sharing links' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER automation_public_link_boundary BEFORE INSERT OR UPDATE ON journal_share_links
FOR EACH ROW EXECUTE FUNCTION automation_guard_public_link();

CREATE FUNCTION automation_guard_demo_identity() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_TABLE_NAME='automation_demo_users' THEN
        IF TG_OP='UPDATE' THEN
            RAISE EXCEPTION 'Demo ownership is immutable' USING ERRCODE='23514';
        END IF;
        IF NOT EXISTS(SELECT 1 FROM users WHERE id=NEW.user_id AND principal_type='HUMAN'
            AND email='demo-' || NEW.user_id::text || '@automation.invalid'
            AND google_sub IS NULL AND apple_sub IS NULL) THEN
            RAISE EXCEPTION 'Invalid demo identity' USING ERRCODE='23514';
        END IF;
    ELSE
        IF EXISTS(SELECT 1 FROM automation_demo_users WHERE user_id=OLD.id)
            AND (NEW.email IS DISTINCT FROM OLD.email OR NEW.password IS DISTINCT FROM OLD.password
                OR NEW.principal_type IS DISTINCT FROM OLD.principal_type
                OR NEW.is_platform_user IS DISTINCT FROM OLD.is_platform_user
                OR NEW.google_sub IS DISTINCT FROM OLD.google_sub OR NEW.apple_sub IS DISTINCT FROM OLD.apple_sub) THEN
            RAISE EXCEPTION 'Demo credentials and identity are immutable' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER automation_demo_identity BEFORE INSERT OR UPDATE ON automation_demo_users
FOR EACH ROW EXECUTE FUNCTION automation_guard_demo_identity();
CREATE TRIGGER automation_user_identity BEFORE UPDATE ON users
FOR EACH ROW EXECUTE FUNCTION automation_guard_demo_identity();

CREATE FUNCTION automation_guard_role() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.role<>'MEMBER' AND EXISTS(SELECT 1 FROM automation_demo_users WHERE user_id=NEW.user_id) THEN
        RAISE EXCEPTION 'Demo users cannot receive privileged roles' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER automation_demo_role BEFORE INSERT OR UPDATE ON user_roles
FOR EACH ROW EXECUTE FUNCTION automation_guard_role();

ALTER TABLE outbound_emails DROP CONSTRAINT outbound_emails_status_check;
ALTER TABLE outbound_emails ADD CONSTRAINT outbound_emails_status_check
    CHECK (status IN ('PENDING','RUNNING','SENT','FAILED','SUPPRESSED'));
CREATE FUNCTION automation_suppress_email() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS(SELECT 1 FROM automation_demo_users d JOIN users u ON u.id=d.user_id
        WHERE d.user_id=NEW.recipient_user_id OR lower(u.email)=lower(NEW.to_email)) THEN
        NEW.status='SUPPRESSED';
        NEW.last_error='Demo delivery suppressed';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER automation_demo_email BEFORE INSERT ON outbound_emails
FOR EACH ROW EXECUTE FUNCTION automation_suppress_email();
