-- This file should undo anything in `up.sql`
ALTER TABLE thought_outputs DROP COLUMN category_id;
DROP TABLE categories;
