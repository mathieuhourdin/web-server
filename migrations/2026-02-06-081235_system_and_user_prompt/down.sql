-- This file should undo anything in `up.sql`
ALTER TABLE llm_calls DROP COLUMN system_prompt;
ALTER TABLE llm_calls DROP COLUMN user_prompt;