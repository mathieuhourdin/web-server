-- Your SQL goes here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

ALTER TABLE articles
ADD COLUMN publishing_state TEXT NOT NULL DEFAULT 'drft';
