CREATE TABLE relationships (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    requester_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    relationship_type TEXT NOT NULL CHECK (relationship_type IN ('FOLLOW')),
    status TEXT NOT NULL CHECK (status IN ('PENDING', 'ACCEPTED', 'REJECTED', 'BLOCKED')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (requester_user_id, target_user_id, relationship_type),
    CHECK (requester_user_id <> target_user_id)
);

CREATE INDEX idx_relationships_requester_user_id ON relationships(requester_user_id);
CREATE INDEX idx_relationships_target_user_id ON relationships(target_user_id);

CREATE TABLE journal_grants (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    journal_id UUID NOT NULL REFERENCES journals(id) ON DELETE CASCADE,
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    grantee_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    grantee_scope TEXT NULL CHECK (grantee_scope IN ('ALL_ACCEPTED_FOLLOWERS')),
    access_level TEXT NOT NULL CHECK (access_level IN ('READ')),
    status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'REVOKED')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CHECK (
        (grantee_user_id IS NOT NULL AND grantee_scope IS NULL)
        OR (grantee_user_id IS NULL AND grantee_scope IS NOT NULL)
    ),
    CHECK (owner_user_id <> grantee_user_id)
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

SELECT diesel_manage_updated_at('relationships');
SELECT diesel_manage_updated_at('journal_grants');
