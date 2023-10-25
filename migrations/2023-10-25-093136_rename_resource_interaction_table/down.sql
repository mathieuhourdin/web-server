-- This file should undo anything in `up.sql`
ALTER TABLE resource_relations RENAME TO thought_input_usages;

ALTER TABLE thought_input_usages DROP CONSTRAINT resource_relations_origin_resource_id_fkey;

UPDATE thought_input_usages  
SET origin_resource_id = interactions.id
FROM interactions
WHERE interactions.resource_id = thought_input_usages.origin_resource_id AND interactions.interaction_type = 'inpt';

ALTER TABLE thought_input_usages RENAME origin_resource_id TO thought_input_id;

ALTER TABLE thought_input_usages
ADD CONSTRAINT thought_input_usages_input_interaction_id_fkey
FOREIGN KEY (thought_input_id)
REFERENCES interactions(id);

ALTER TABLE thought_input_usages RENAME target_resource_id TO resource_id;
ALTER TABLE thought_input_usages RENAME relation_comment TO usage_reason;

ALTER TABLE thought_input_usages DROP COLUMN relation_type;
