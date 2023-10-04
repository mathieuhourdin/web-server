-- Your SQL goes here
ALTER TABLE thought_outputs RENAME TO interactions;
ALTER TABLE thought_input_usages DROP CONSTRAINT thought_input_usages_thought_input_id_fkey;
ALTER TABLE thought_input_usages
  ADD CONSTRAINT thought_input_usages_input_interaction_id_fkey
  FOREIGN KEY (thought_input_id)
  REFERENCES interactions(id);
DROP TABLE thought_inputs;
