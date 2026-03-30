ALTER TABLE posts
ADD COLUMN source_trace_id UUID NULL REFERENCES traces(id) ON DELETE SET NULL;

CREATE INDEX idx_posts_source_trace_id ON posts(source_trace_id);

CREATE TABLE post_grants (
    id UUID PRIMARY KEY,
    post_id UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    grantee_user_id UUID NULL REFERENCES users(id) ON DELETE CASCADE,
    grantee_scope TEXT NULL CHECK (grantee_scope IN ('ALL_ACCEPTED_FOLLOWERS')),
    access_level TEXT NOT NULL CHECK (access_level IN ('READ')),
    status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'REVOKED')),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CHECK (
        (grantee_user_id IS NOT NULL AND grantee_scope IS NULL)
        OR (grantee_user_id IS NULL AND grantee_scope IS NOT NULL)
    )
);

CREATE INDEX idx_post_grants_post_id ON post_grants(post_id);
CREATE INDEX idx_post_grants_owner_user_id ON post_grants(owner_user_id);
CREATE INDEX idx_post_grants_grantee_user_id ON post_grants(grantee_user_id);
CREATE UNIQUE INDEX idx_post_grants_direct_unique
    ON post_grants(post_id, grantee_user_id)
    WHERE grantee_user_id IS NOT NULL;
CREATE UNIQUE INDEX idx_post_grants_scope_unique
    ON post_grants(post_id, grantee_scope)
    WHERE grantee_scope IS NOT NULL;

SELECT diesel_manage_updated_at('post_grants');
