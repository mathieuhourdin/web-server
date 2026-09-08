DROP INDEX IF EXISTS llm_calls_message_id_idx;

ALTER TABLE llm_calls
DROP COLUMN cached_input_tokens_used;

ALTER TABLE llm_calls
DROP COLUMN message_id;
