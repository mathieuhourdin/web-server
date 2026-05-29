-- This file should undo anything in `up.sql`
ALTER TABLE journals
DROP CONSTRAINT IF EXISTS journals_sharing_mode_check,
DROP COLUMN IF EXISTS sharing_mode;
