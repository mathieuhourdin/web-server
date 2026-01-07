-- Your SQL goes here
-- Add CASCADE DELETE to foreign key constraints referencing resources(id)

-- Drop existing foreign key constraints
-- Note: PostgreSQL may auto-generate constraint names, so we check for common variations
ALTER TABLE interactions 
    DROP CONSTRAINT IF EXISTS interactions_resource_id_fkey;

ALTER TABLE comments 
    DROP CONSTRAINT IF EXISTS comments_resource_id_fkey;

ALTER TABLE resource_relations 
    DROP CONSTRAINT IF EXISTS resource_relations_origin_resource_id_fkey,
    DROP CONSTRAINT IF EXISTS resource_relations_target_resource_id_fkey,
    DROP CONSTRAINT IF EXISTS thought_input_usages_resource_id_fkey;

-- Re-add with CASCADE DELETE
ALTER TABLE interactions 
    ADD CONSTRAINT interactions_resource_id_fkey 
    FOREIGN KEY (resource_id) 
    REFERENCES resources(id) 
    ON DELETE CASCADE;

ALTER TABLE comments 
    ADD CONSTRAINT comments_resource_id_fkey 
    FOREIGN KEY (resource_id) 
    REFERENCES resources(id) 
    ON DELETE CASCADE;

ALTER TABLE resource_relations 
    ADD CONSTRAINT resource_relations_origin_resource_id_fkey 
    FOREIGN KEY (origin_resource_id) 
    REFERENCES resources(id) 
    ON DELETE CASCADE;

ALTER TABLE resource_relations 
    ADD CONSTRAINT resource_relations_target_resource_id_fkey 
    FOREIGN KEY (target_resource_id) 
    REFERENCES resources(id) 
    ON DELETE CASCADE;

