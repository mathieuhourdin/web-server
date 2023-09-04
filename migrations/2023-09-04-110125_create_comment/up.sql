-- Your SQL goes here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE comments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    content TEXT NOT NULL DEFAULT '',
    article_id UUID REFERENCES articles(id) NOT NULL,
    comment_type TEXT,
    start_index INTEGER,
    end_index INTEGER,
    parent_id UUID REFERENCES comments(id),
    editing BOOLEAN NOT NULL DEFAULT TRUE,
    author_id UUID REFERENCES users(id) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

SELECT diesel_manage_updated_at('comments');
