-- This file should undo anything in `up.sql`
ALTER TABLE comments DROP CONSTRAINT comments_resource_id_fkey ;
UPDATE comments
SET resource_id = interactions.id
FROM interactions
WHERE comments.resource_id = interactions.resource_id AND interactions.interaction_type = 'outp';
ALTER TABLE comments RENAME resource_id TO thought_output_id ;
ALTER TABLE comments
ADD CONSTRAINT comments_article_id_fkey 
FOREIGN KEY (thought_output_id )
REFERENCES interactions(id);
