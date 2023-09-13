-- Your SQL goes here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE thought_input_usages (
    id UUID NOT NULL DEFAULT uuid_generate_v4(),
    thought_input_id UUID REFERENCES thought_inputs(id) NOT NULL,
    thought_output_id UUID REFERENCES thought_outputs(id) NOT NULL,
    usage_reason TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    PRIMARY KEY(thought_input_id, thought_output_id)
);

SELECT diesel_manage_updated_at('thought_input_usages');
