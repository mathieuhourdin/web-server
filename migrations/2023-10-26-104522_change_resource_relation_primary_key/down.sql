-- This file should undo anything in `up.sql`
ALTER TABLE resource_relations DROP CONSTRAINT resource_relations_pkey;
ALTER TABLE resource_relations
ADD CONSTRAINT thought_input_usages_pkey PRIMARY KEY (target_resource_id, origin_resource_id);
