-- Your SQL goes here
UPDATE interactions SET interaction_type = 'outp' WHERE interaction_type IS NULL;
UPDATE resources SET is_external = (interaction_type = 'inpt') FROM interactions WHERE resources.id = interactions.resource_id;
