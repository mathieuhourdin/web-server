-- Your SQL goes here
ALTER TABLE thought_input_usages RENAME TO resource_relations;

ALTER TABLE resource_relations ADD COLUMN user_id UUID REFERENCES users(id);

ALTER TABLE resource_relations DROP CONSTRAINT thought_input_usages_input_interaction_id_fkey;

UPDATE resource_relations
SET thought_input_id = interactions.resource_id, user_id = interactions.interaction_user_id
FROM interactions
WHERE interactions.id = resource_relations.thought_input_id;

ALTER TABLE resource_relations ALTER COLUMN user_id SET NOT NULL;
ALTER TABLE resource_relations RENAME thought_input_id TO origin_resource_id;

ALTER TABLE resource_relations
ADD CONSTRAINT resource_relations_origin_resource_id_fkey
FOREIGN KEY (origin_resource_id)
REFERENCES resources(id);

ALTER TABLE resource_relations RENAME resource_id TO target_resource_id;
ALTER TABLE resource_relations RENAME usage_reason TO relation_comment;

ALTER TABLE resource_relations ADD COLUMN relation_type TEXT NOT NULL DEFAULT 'bibl';
