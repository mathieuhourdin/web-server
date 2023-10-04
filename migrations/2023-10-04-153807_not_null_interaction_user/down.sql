-- This file should undo anything in `up.sql`
ALTER TABLE interactions ALTER COLUMN interaction_user_id DROP NOT NULL;
