-- This file should undo anything in `up.sql`
ALTER TABLE thought_outputs RENAME TO articles;

ALTER TABLE comments RENAME COLUMN thought_output_id TO article_id;
