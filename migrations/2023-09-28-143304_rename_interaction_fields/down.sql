-- This file should undo anything in `up.sql`
ALTER TABLE thought_outputs DROP COLUMN interaction_comment;
ALTER TABLE thought_outputs DROP COLUMN interaction_date;
ALTER TABLE thought_outputs DROP COLUMN interaction_type;
ALTER TABLE thought_outputs DROP COLUMN interaction_is_public;
ALTER TABLE thought_outputs ADD COLUMN url_slug TEXT;
ALTER TABLE thought_outputs RENAME COLUMN interaction_user_id TO author_id;
ALTER TABLE thought_outputs RENAME COLUMN resource_parent_id TO parent_id ;
ALTER TABLE thought_outputs RENAME COLUMN interaction_progress TO progress;
ALTER TABLE thought_outputs RENAME COLUMN resource_maturing_state TO maturing_state;
ALTER TABLE thought_outputs RENAME COLUMN resource_publishing_state TO publishing_state;


ALTER TABLE thought_inputs DROP COLUMN resource_publishing_state;
ALTER TABLE thought_inputs DROP COLUMN resource_maturing_state;
ALTER TABLE thought_inputs RENAME COLUMN interaction_progress TO input_progress;
ALTER TABLE thought_inputs RENAME COLUMN interaction_date TO input_date;
ALTER TABLE thought_inputs RENAME COLUMN interaction_is_public TO input_is_public;
ALTER TABLE thought_inputs RENAME COLUMN interaction_comment TO input_comment;
ALTER TABLE thought_inputs RENAME COLUMN interaction_user_id TO input_user_id;
