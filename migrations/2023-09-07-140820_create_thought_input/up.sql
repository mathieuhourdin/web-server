-- Your SQL goes here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE thought_inputs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    resource_title TEXT NOT NULL DEFAULT '',
    resource_author_name TEXT NOT NULL DEFAULT '',
    resource_type TEXT,
    resource_link TEXT,
    resource_image_link TEXT,
    resource_comment TEXT NOT NULL DEFAULT '',
    input_progress INTEGER NOT NULL DEFAULT 0,
    input_date TIMESTAMP,
    input_comment TEXT NOT NULL DEFAULT '',
    input_is_public BOOLEAN NOT NULL DEFAULT TRUE,
    input_user_id UUID REFERENCES users(id) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

SELECT diesel_manage_updated_at('thought_inputs');
