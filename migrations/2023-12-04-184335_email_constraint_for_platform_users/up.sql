-- Your SQL goes here
ALTER TABLE users DROP CONSTRAINT users_email_key;
ALTER TABLE users DROP CONSTRAINT users_handle_key;
CREATE UNIQUE INDEX platform_users_unique_email_key ON users (email)
    WHERE is_platform_user;
CREATE UNIQUE INDEX platform_users_unique_handle_key ON users (handle)
    WHERE is_platform_user;
