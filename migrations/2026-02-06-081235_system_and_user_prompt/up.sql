-- Your SQL goes here
ALTER TABLE llm_calls ADD COLUMN system_prompt TEXT NOT NULL DEFAULT '';
ALTER TABLE llm_calls ADD COLUMN user_prompt TEXT NOT NULL DEFAULT '';
