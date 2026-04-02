CREATE TABLE user_post_states (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    post_id UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    first_seen_at TIMESTAMP NOT NULL,
    last_seen_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT user_post_states_user_post_unique UNIQUE (user_id, post_id)
);

CREATE INDEX user_post_states_user_id_last_seen_at_idx
    ON user_post_states (user_id, last_seen_at DESC);

CREATE INDEX user_post_states_post_id_idx
    ON user_post_states (post_id);
