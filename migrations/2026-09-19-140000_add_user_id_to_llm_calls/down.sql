DROP INDEX IF EXISTS llm_calls_user_id_created_at_idx;

ALTER TABLE llm_calls
    DROP COLUMN user_id;
