CREATE TABLE journal_grants (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    journal_id UUID NOT NULL REFERENCES journals(id) ON DELETE CASCADE,
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    grantee_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    grantee_scope TEXT,
    access_level TEXT NOT NULL DEFAULT 'READ',
    status TEXT NOT NULL DEFAULT 'ACTIVE',
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    CONSTRAINT journal_grants_target_check CHECK (
        (grantee_user_id IS NOT NULL AND grantee_scope IS NULL)
        OR (grantee_user_id IS NULL AND grantee_scope IS NOT NULL)
    ),
    CONSTRAINT journal_grants_grantee_scope_check CHECK (
        grantee_scope IS NULL OR grantee_scope IN ('ALL_ACCEPTED_FOLLOWERS', 'ALL_PLATFORM_USERS')
    ),
    CONSTRAINT journal_grants_access_level_check CHECK (access_level IN ('READ')),
    CONSTRAINT journal_grants_status_check CHECK (status IN ('ACTIVE', 'REVOKED'))
);

CREATE INDEX idx_journal_grants_journal_id ON journal_grants(journal_id);
CREATE INDEX idx_journal_grants_owner_user_id ON journal_grants(owner_user_id);
CREATE INDEX idx_journal_grants_grantee_user_id ON journal_grants(grantee_user_id);
CREATE UNIQUE INDEX idx_journal_grants_direct_unique
    ON journal_grants(journal_id, grantee_user_id)
    WHERE grantee_user_id IS NOT NULL;
CREATE UNIQUE INDEX idx_journal_grants_scope_unique
    ON journal_grants(journal_id, grantee_scope)
    WHERE grantee_scope IS NOT NULL;

SELECT diesel_manage_updated_at('journal_grants');

INSERT INTO journal_grants (
    id,
    journal_id,
    owner_user_id,
    grantee_user_id,
    grantee_scope,
    access_level,
    status,
    created_at,
    updated_at
)
SELECT
    uuid_generate_v4(),
    journal_id,
    owner_user_id,
    grantee_user_id,
    NULL,
    'READ',
    CASE WHEN status IN ('ACTIVE', 'SUGGESTED') THEN 'ACTIVE' ELSE 'REVOKED' END,
    created_at,
    updated_at
FROM journal_sharing_policies;

DROP TABLE IF EXISTS journal_sharing_policies;
