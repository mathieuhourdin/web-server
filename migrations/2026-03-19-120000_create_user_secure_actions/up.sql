CREATE TABLE user_secure_actions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action_type TEXT NOT NULL,
    payload TEXT,
    secret_hash TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    used_at TIMESTAMP,
    revoked_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT user_secure_actions_action_type_check
        CHECK (action_type IN ('PASSWORD_RESET'))
);

CREATE INDEX user_secure_actions_user_id_idx
    ON user_secure_actions (user_id);

CREATE INDEX user_secure_actions_lookup_idx
    ON user_secure_actions (action_type, expires_at)
    WHERE used_at IS NULL AND revoked_at IS NULL;
