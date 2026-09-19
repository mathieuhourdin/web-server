ALTER TABLE llm_calls
    ADD COLUMN user_id UUID NULL REFERENCES users(id) ON DELETE CASCADE;

CREATE INDEX llm_calls_user_id_created_at_idx
    ON llm_calls (user_id, created_at DESC)
    WHERE user_id IS NOT NULL;
