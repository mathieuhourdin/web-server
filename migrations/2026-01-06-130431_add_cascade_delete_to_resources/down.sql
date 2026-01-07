-- This file should undo anything in `up.sql`
-- Remove CASCADE DELETE from foreign key constraints (restore to default NO ACTION)

-- Drop CASCADE constraints
ALTER TABLE interactions 
    DROP CONSTRAINT IF EXISTS interactions_resource_id_fkey;

ALTER TABLE comments 
    DROP CONSTRAINT IF EXISTS comments_resource_id_fkey;

ALTER TABLE resource_relations 
    DROP CONSTRAINT IF EXISTS resource_relations_origin_resource_id_fkey,
    DROP CONSTRAINT IF EXISTS resource_relations_target_resource_id_fkey;

-- Re-add without CASCADE (default behavior: NO ACTION)
ALTER TABLE interactions 
    ADD CONSTRAINT interactions_resource_id_fkey 
    FOREIGN KEY (resource_id) 
    REFERENCES resources(id);

ALTER TABLE comments 
    ADD CONSTRAINT comments_resource_id_fkey 
    FOREIGN KEY (resource_id) 
    REFERENCES resources(id);

ALTER TABLE resource_relations 
    ADD CONSTRAINT resource_relations_origin_resource_id_fkey 
    FOREIGN KEY (origin_resource_id) 
    REFERENCES resources(id);

ALTER TABLE resource_relations 
    ADD CONSTRAINT resource_relations_target_resource_id_fkey 
    FOREIGN KEY (target_resource_id) 
    REFERENCES resources(id);

