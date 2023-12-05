-- This file should undo anything in `up.sql`
ALTER TABLE users
ADD CONSTRAINT users_email_key UNIQUE (email);
ALTER TABLE users
ADD CONSTRAINT users_handle_key UNIQUE (handle);
DROP INDEX platform_users_unique_email_key;
DROP INDEX platform_users_unique_handle_key;
