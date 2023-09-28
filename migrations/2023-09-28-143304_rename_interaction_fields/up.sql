-- Your SQL goes here
ALTER TABLE thought_outputs ADD COLUMN interaction_comment TEXT;
ALTER TABLE thought_outputs ADD COLUMN interaction_date TIMESTAMP;
ALTER TABLE thought_outputs ADD COLUMN interaction_type TEXT;
ALTER TABLE thought_outputs ADD COLUMN interaction_is_public BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE thought_outputs DROP COLUMN url_slug;
ALTER TABLE thought_outputs RENAME COLUMN author_id TO interaction_user_id;
ALTER TABLE thought_outputs RENAME COLUMN parent_id TO resource_parent_id;
ALTER TABLE thought_outputs RENAME COLUMN progress TO interaction_progress;
ALTER TABLE thought_outputs RENAME COLUMN maturing_state TO resource_maturing_state;
ALTER TABLE thought_outputs RENAME COLUMN publishing_state TO resource_publishing_state;


ALTER TABLE thought_inputs ADD COLUMN resource_publishing_state TEXT;
ALTER TABLE thought_inputs ADD COLUMN resource_maturing_state TEXT;
ALTER TABLE thought_inputs RENAME COLUMN input_progress TO interaction_progress;
ALTER TABLE thought_inputs RENAME COLUMN input_date TO interaction_date;
ALTER TABLE thought_inputs RENAME COLUMN input_is_public TO interaction_is_public;
ALTER TABLE thought_inputs RENAME COLUMN input_comment TO interaction_comment;
ALTER TABLE thought_inputs RENAME COLUMN input_user_id TO interaction_user_id;
