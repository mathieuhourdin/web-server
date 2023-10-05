-- This file should undo anything in `up.sql`
ALTER TABLE interactions DROP COLUMN resource_id;
DROP TABLE resources;
