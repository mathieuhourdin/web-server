ALTER TABLE messages
ADD COLUMN suggested_actions JSONB NOT NULL DEFAULT '[]'::jsonb;
