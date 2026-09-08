ALTER TABLE llm_calls
ADD COLUMN message_id UUID REFERENCES messages(id) ON DELETE CASCADE;

ALTER TABLE llm_calls
ADD COLUMN cached_input_tokens_used INTEGER NOT NULL DEFAULT 0;

CREATE INDEX llm_calls_message_id_idx
ON llm_calls (message_id)
WHERE message_id IS NOT NULL;
