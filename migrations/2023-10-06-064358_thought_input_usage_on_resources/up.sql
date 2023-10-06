-- Your SQL goes here
ALTER TABLE thought_input_usages DROP CONSTRAINT thought_input_usages_thought_output_id_fkey;

UPDATE thought_input_usages
SET thought_output_id = interactions.resource_id
FROM interactions
WHERE interactions.id = thought_input_usages.thought_output_id;

ALTER TABLE thought_input_usages RENAME COLUMN thought_output_id TO resource_id;

ALTER TABLE thought_input_usages
ADD CONSTRAINT thought_input_usages_resource_id_fkey
FOREIGN KEY (resource_id)
REFERENCES resources(id);
