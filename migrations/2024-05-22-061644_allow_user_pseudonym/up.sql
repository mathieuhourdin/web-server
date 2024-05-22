-- Your SQL goes here
ALTER TABLE users ADD COLUMN pseudonym TEXT;
ALTER TABLE users ADD COLUMN pseudonymized BOOLEAN NOT NULL DEFAULT false;
