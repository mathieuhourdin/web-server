-- Your SQL goes here
ALTER TABLE comments DROP CONSTRAINT comments_article_id_fkey;
UPDATE comments
SET thought_output_id = interactions.resource_id
FROM interactions
WHERE thought_output_id = interactions.id;
ALTER TABLE comments RENAME thought_output_id TO resource_id;
ALTER TABLE comments
ADD CONSTRAINT comments_resource_id_fkey 
FOREIGN KEY (resource_id)
REFERENCES resources(id);
