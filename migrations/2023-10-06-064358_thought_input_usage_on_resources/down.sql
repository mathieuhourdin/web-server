-- This file should undo anything in `up.sql`
ALTER TABLE thought_input_usages DROP CONSTRAINT thought_input_usages_resource_id_fkey;

UPDATE thought_input_usages
SET resource_id = interactions.id
FROM interactions
WHERE interactions.resource_id = thought_input_usages.resource_id;

ALTER TABLE thought_input_usages RENAME COLUMN resource_id TO thought_output_id;

ALTER TABLE thought_input_usages
ADD CONSTRAINT thought_input_usages_thought_output_id_fkey
FOREIGN KEY (thought_output_id)
REFERENCES interactions(id);
