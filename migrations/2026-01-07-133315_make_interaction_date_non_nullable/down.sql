-- This file should undo anything in `up.sql`
ALTER TABLE interactions 
ALTER COLUMN interaction_date DROP NOT NULL,
ALTER COLUMN interaction_date DROP DEFAULT;