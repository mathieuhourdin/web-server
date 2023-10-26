-- Your SQL goes here
ALTER TABLE resource_relations DROP CONSTRAINT thought_input_usages_pkey;
ALTER TABLE resource_relations
ADD CONSTRAINT resource_relations_pkey PRIMARY KEY (id);
