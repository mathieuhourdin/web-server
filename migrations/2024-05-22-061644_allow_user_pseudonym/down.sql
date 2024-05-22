-- This file should undo anything in `up.sql`
ALTER TABLE users DROP COLUMN pseudonym;
ALTER TABLE users DROP COLUMN pseudonymized;
