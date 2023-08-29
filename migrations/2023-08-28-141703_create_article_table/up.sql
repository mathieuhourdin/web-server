-- Your SQL goes here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE articles (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    potential_improvements TEXT NOT NULL DEFAULT '',
    author_id UUID REFERENCES users(id),
    progress INT NOT NULL DEFAULT 0,
    maturing_state TEXT NOT NULL DEFAULT 'idea',
    parent_id UUID REFERENCES articles(id),
    gdoc_url TEXT,
    image_url TEXT,
    url_slug TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);
 
SELECT diesel_manage_updated_at('articles');
