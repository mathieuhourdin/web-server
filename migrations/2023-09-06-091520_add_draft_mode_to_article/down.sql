-- This file should undo anything in `up.sql`
SELECT 1;
ALTER TABLE articles
DROP COLUMN publishing_state;
