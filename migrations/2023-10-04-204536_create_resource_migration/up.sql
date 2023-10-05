-- Your SQL goes here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE resources (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title TEXT NOT NULL DEFAULT '',
    subtitle TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    external_content_url TEXT,
    comment TEXT,
    image_url TEXT,
    resource_type TEXT NOT NULL DEFAULT 'atcl',
    maturing_state TEXT NOT NULL DEFAULT 'idea',
    publishing_state TEXT NOT NULL DEFAULT 'drft',
    category_id UUID REFERENCES categories(id),
    is_external BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    temporary_interaction_id UUID
);
 
SELECT diesel_manage_updated_at('resources');

INSERT INTO
  resources (title, subtitle, content, external_content_url, comment, image_url, resource_type, maturing_state, publishing_state, category_id, created_at, updated_at, temporary_interaction_id)
  SELECT resource_title as title, resource_subtitle as subtitle, resource_content as content, resource_external_content_url as external_content_url, resource_comment as comment, resource_image_url as image_url, resource_type as resource_type, resource_maturing_state as maturing_state, resource_publishing_state as publishing_state, resource_category_id as category_id, created_at, updated_at, id as temporary_interaction_id
  FROM interactions;

ALTER TABLE interactions ADD COLUMN resource_id UUID REFERENCES resources(id);
UPDATE interactions
SET resource_id = resources.id
FROM resources
WHERE interactions.id = resources.temporary_interaction_id;

ALTER TABLE resources DROP COLUMN temporary_interaction_id;
