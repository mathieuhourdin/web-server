-- This file should undo anything in `up.sql`
ALTER TABLE interactions RENAME TO thought_outputs;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE thought_inputs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4()
);

INSERT INTO thought_inputs (id)
SELECT id
FROM thought_outputs
WHERE thought_outputs.interaction_type = 'inpt';

ALTER TABLE thought_input_usages DROP CONSTRAINT thought_input_usages_input_interaction_id_fkey;

ALTER TABLE thought_input_usages
  ADD CONSTRAINT thought_input_usages_thought_input_id_fkey
  FOREIGN KEY (thought_input_id)
  REFERENCES thought_inputs(id);

